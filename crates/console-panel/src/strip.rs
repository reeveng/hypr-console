//! Which tabs the strip has room for.
//!
//! The card is one width whatever is written on its tabs, so a strip of five
//! long words cannot ask for more than the card was going to be. What will not
//! fit is reached with the arrows at either end, which are the shoulders said
//! in the other language.

use crate::canvas::{Area, AreaLayout, Canvas};
use crate::marks;
use console_core_color::{Oklch, palette::Wearing};
use console_core_fonts::{EM, TextStyle};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Font, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Lines, Run, at_most, measure_text};
use std::ops::Range;

use crate::keys::TabMotion;

pub const HEIGHT: i32 = EM * 8 / 3;
pub(crate) const ROUND: u32 = (EM * 2 / 3).unsigned_abs();
const LABEL_PAD: u32 = (EM * 8 / 9).unsigned_abs();
const MIN_TAB: i32 = EM * 40 / 9;

pub const GAP: i32 = EM * 2 / 9;
pub const MARGIN: i32 = EM * 7 / 9;
pub const PAD: i32 = EM / 3;

pub const PICTURE: i32 = EM * 16 / 9;

pub const SLEEVE: i32 = EM * 88 / 9;

pub const ANSWER: i32 = EM * 25 / 3;

pub const PRESSED: i32 = EM * 5 / 3;

pub const EDGE: i32 = EM / 6;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Control {
  Ready,
  Focused,
  Unavailable,
}

pub(crate) fn centred(
  said: &str,
  measured: Size<u32>,
  within: &Panel,
  weight: Weight,
  face: Font,
  ink: Oklch,
) -> Result<Text, Never> {
  let Ok(wide) = fitted::<u32, i32>(within.size.width);
  let Ok(tall) = fitted::<u32, i32>(within.size.height);
  let Ok(said_wide) = fitted::<u32, i32>(measured.width);
  let Ok(said_tall) = fitted::<u32, i32>(measured.height);
  let down = within
    .at
    .y
    .saturating_add(tall.saturating_sub(said_tall).saturating_div(2));

  Ok(Text {
    at: Point {
      x: within
        .at
        .x
        .saturating_add(wide.saturating_sub(said_wide).max(0).saturating_div(2)),
      y: down,
    },
    width: measured.width.saturating_add(2).min(within.size.width),
    said: said.to_string(),
    weight,
    font: face,
    ink,
  })
}

pub(crate) fn control(
  at: Point<i32>,
  size: Size<u32>,
  said: &str,
  measured: Size<u32>,
  state: Control,
  colors: &Wearing,
) -> Result<(Panel, Text), Never> {
  let (fill, ink) = match state {
    Control::Ready => (colors.night, colors.text),
    Control::Focused => (colors.pink, colors.night),
    Control::Unavailable => (colors.night, colors.edge),
  };
  let panel = Panel {
    at,
    size,
    round: Round(ROUND),
    fill,
    edge: Edge::None,
  };
  let Ok(face) = TextStyle::Body.font();
  let Ok(words) = centred(said, measured, &panel, Weight::Bold, face, ink);
  Ok((panel, words))
}

pub fn paint_control(
  shapes: &mut Vec<Shape>,
  area: Area,
  label: &str,
  state: Control,
  colors: &Wearing,
) -> Result<(), Never> {
  let Ok(size) = area.pixel_size();
  let Ok(face) = TextStyle::Body.font();
  let Ok(measured) = measure_text(
    Run {
      said: label,
      weight: Weight::Bold,
      width: size.width,
    },
    &face,
  );
  let Ok((panel, words)) = control(area.origin, size, label, measured, state, colors);
  shapes.extend([Shape::Panel(panel), Shape::Text(words)]);
  Ok(())
}

#[derive(Clone, Copy)]
pub enum TabFace {
  Other,
  Selected,
  Focused,
  Leaving,
}

impl TabFace {
  fn weight(self) -> Result<Weight, Never> {
    Ok(match self {
      Self::Other => Weight::Plain,
      Self::Selected | Self::Focused | Self::Leaving => Weight::Bold,
    })
  }
}

pub(crate) fn tab_shapes(
  shapes: &mut Vec<Shape>,
  cell: Panel,
  said: &str,
  measured: Size<u32>,
  font: Font,
  state: TabFace,
  colors: &Wearing,
) -> Result<(), Never> {
  let (fill, ink) = match state {
    TabFace::Other => (None, colors.soft),
    TabFace::Selected => (Some(colors.pink), colors.night),
    TabFace::Focused => (Some(colors.fill), colors.text),
    TabFace::Leaving => (Some(colors.ground), colors.text),
  };

  match fill {
    Some(tint) => {
      let wide = measured
        .width
        .saturating_add(LABEL_PAD.saturating_mul(2))
        .min(cell.size.width);
      let Ok(offset) = fitted::<_, i32>(cell.size.width.saturating_sub(wide).saturating_div(2));
      shapes.push(Shape::Panel(Panel {
        at: Point {
          x: cell.at.x.saturating_add(offset),
          y: cell.at.y,
        },
        size: Size {
          width: wide,
          height: cell.size.height,
        },
        fill: tint,
        ..cell
      }));
    }
    None => {}
  }

  let Ok(weight) = state.weight();
  let Ok(words) = centred(said, measured, &cell, weight, font, ink);
  shapes.push(Shape::Text(words));
  Ok(())
}

pub fn paint_tab(
  shapes: &mut Vec<Shape>,
  area: Area,
  label: &str,
  state: TabFace,
  colors: &Wearing,
) -> Result<(), Never> {
  let Ok(font) = TextStyle::Headline.font();
  let Ok(size) = area.pixel_size();
  let Ok(weight) = state.weight();
  let run = Run {
    said: label,
    weight,
    width: size.width.saturating_sub(LABEL_PAD.saturating_mul(2)),
  };
  let Ok(visible) = at_most(run, &font, Lines(1));
  let Ok(measured) = measure_text(
    Run {
      said: &visible,
      ..run
    },
    &font,
  );
  let panel = Panel {
    at: area.origin,
    size,
    round: Round(ROUND),
    fill: colors.panel,
    edge: Edge::None,
  };
  tab_shapes(shapes, panel, &visible, measured, font, state, colors)
}

pub struct Header<'a, A> {
  pub title: &'a str,
  pub tabs: &'a [&'a str],
  pub selected: u32,
  pub select: fn(u32) -> A,
  pub previous: A,
  pub next: A,
  pub close: A,
}

pub fn header<A: Copy + PartialEq>(
  canvas: &mut Canvas<A>,
  area: Area,
  header: &Header<'_, A>,
  colors: &Wearing,
) -> Result<(), Never> {
  let Ok(right) = area.right();
  let y = area.origin.y;
  let close_x = right.saturating_sub(HEIGHT);
  let Ok(()) = header_control(
    canvas,
    Point { x: close_x, y },
    marks::SHUT,
    Some(header.close),
    colors,
  );
  let Ok(many) = fitted::<_, u32>(header.tabs.len());
  let Ok(font) = TextStyle::Headline.font();

  match header.tabs {
    [] | [_] => {
      let Ok(width) = fitted::<_, u32>(
        close_x
          .saturating_sub(area.origin.x)
          .saturating_sub(GAP)
          .max(0),
      );
      let said = match header.tabs.first() {
        Some(tab) => *tab,
        None => header.title,
      };
      let run = Run {
        said,
        weight: Weight::Bold,
        width,
      };
      let Ok(visible) = at_most(run, &font, Lines(1));
      let Ok(measured) = measure_text(
        Run {
          said: &visible,
          ..run
        },
        &font,
      );
      let Ok(tall) = fitted::<_, i32>(measured.height);
      canvas.frame.shapes.push(Shape::Text(Text {
        at: Point {
          x: area.origin.x,
          y: y.saturating_add(HEIGHT.saturating_sub(tall).saturating_div(2)),
        },
        width,
        said: visible,
        weight: Weight::Bold,
        font,
        ink: colors.text,
      }));
    }
    [_, _, ..] => {
      let next_x = close_x.saturating_sub(GAP).saturating_sub(HEIGHT);
      let Ok(()) = header_control(
        canvas,
        area.origin,
        marks::BEFORE,
        (header.selected > 0).then_some(header.previous),
        colors,
      );
      let Ok(()) = header_control(
        canvas,
        Point { x: next_x, y },
        marks::AFTER,
        (header.selected.saturating_add(1) < many).then_some(header.next),
        colors,
      );
      let x = area.origin.x.saturating_add(HEIGHT).saturating_add(GAP);
      let tab_area = Area {
        origin: Point { x, y },
        size: Size {
          width: next_x.saturating_sub(GAP).saturating_sub(x).max(0),
          height: HEIGHT,
        },
      };
      let Ok(()) = header_tabs(
        canvas,
        tab_area,
        header,
        Tabs {
          many,
          here: header.selected,
          from: 0,
          fits: tab_area
            .size
            .width
            .saturating_div(MIN_TAB)
            .unsigned_abs()
            .max(1),
        },
        colors,
      );
    }
  }

  Ok(())
}

pub(crate) fn header_control<A: Copy + PartialEq>(
  canvas: &mut Canvas<A>,
  at: Point<i32>,
  said: &str,
  action: Option<A>,
  colors: &Wearing,
) -> Result<(), Never> {
  let area = Area {
    origin: at,
    size: Size {
      width: HEIGHT,
      height: HEIGHT,
    },
  };
  let state = match action {
    Some(key) => {
      let Ok(()) = canvas.hit(area, key);

      match (canvas.interaction.focus == action || canvas.interaction.hover == action)
        .then_some(())
      {
        Some(()) => Control::Focused,
        None => Control::Ready,
      }
    }
    None => Control::Unavailable,
  };
  paint_control(&mut canvas.frame.shapes, area, said, state, colors)
}

fn header_tabs<A: Copy + PartialEq>(
  canvas: &mut Canvas<A>,
  area: Area,
  header: &Header<'_, A>,
  window: Tabs,
  colors: &Wearing,
) -> Result<(), Never> {
  let Ok(visible) = visible_range(window);
  let Ok(count) = fitted::<_, i32>(visible.end.saturating_sub(visible.start).max(1));
  let width = area.size.width.saturating_div(count);

  for (column, index) in (0_i32..).zip(visible) {
    let Ok(position) = console_core_number_conversion::index(index);
    let said = match header.tabs.get(position) {
      Some(said) => *said,
      None => break,
    };
    let cell = Area {
      origin: Point {
        x: area.origin.x.saturating_add(width.saturating_mul(column)),
        y: area.origin.y,
      },
      size: Size {
        width,
        height: HEIGHT,
      },
    };
    let state = match (index == header.selected).then_some(()) {
      Some(()) => TabFace::Selected,
      None => TabFace::Other,
    };
    let Ok(()) = paint_tab(&mut canvas.frame.shapes, cell, said, state, colors);
    let Ok(()) = canvas.hit(cell, (header.select)(index));
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
  pub width: i32,
  pub spent: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tabs {
  pub many: u32,
  pub here: u32,
  pub from: u32,
  pub fits: u32,
}

pub fn room(card: Card) -> Result<i32, Never> {
  Ok(
    card
      .width
      .saturating_sub(2i32.saturating_mul(EDGE))
      .saturating_sub(2i32.saturating_mul(MARGIN))
      .saturating_sub(2i32.saturating_mul(PAD))
      .saturating_sub(card.spent),
  )
}

pub fn fits(room: i32, cell: Cell) -> Result<u32, Never> {
  let each = cell.0.saturating_add(GAP);

  let Ok(many) = fitted(room.saturating_add(GAP).saturating_div(each).max(1));

  Ok(match ::core::primitive::bool::then_some(each > 0, ()) {
    ::core::option::Option::Some(()) => many,
    ::core::option::Option::None => 1,
  })
}

pub fn visible_range(tabs: Tabs) -> Result<Range<u32>, Never> {
  match ::core::primitive::bool::then_some(tabs.fits >= tabs.many, ()) {
    ::core::option::Option::Some(()) => return Ok(0..tabs.many),
    ::core::option::Option::None => {}
  }

  let least = tabs.here.saturating_add(1).saturating_sub(tabs.fits);
  let first = tabs.from.min(tabs.here).max(least);
  let first_2 = first.min(tabs.many.saturating_sub(tabs.fits));

  Ok(first_2..first_2.saturating_add(tabs.fits))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
  Tab(u32),
  Outside,
}

pub fn switched(tabs: u32, from: Stop, motion: TabMotion) -> Result<Option<u32>, Never> {
  let many = match std::num::NonZeroU32::new(tabs) {
    Some(many) => many,
    None => return Ok(None),
  };
  let last = tabs.saturating_sub(1);
  let at = match (from, motion) {
    (_, TabMotion::Number(number)) => {
      return Ok(number.checked_sub(1).filter(|index| *index < tabs));
    }
    (Stop::Outside, TabMotion::Next) => last,
    (Stop::Outside, TabMotion::Previous(_)) => 0,
    (Stop::Tab(index), TabMotion::Next | TabMotion::Previous(_)) => index.min(last),
  };
  let offset = match motion {
    TabMotion::Next => 1,
    TabMotion::Previous(count) => tabs.saturating_sub(count % many),
    TabMotion::Number(_) => return Ok(None),
  };

  let wrapped =
    u64::from(at).saturating_add(u64::from(offset)) % std::num::NonZeroU64::from(many);
  let Ok(index) = fitted::<u64, u32>(wrapped);
  Ok(Some(index))
}

#[cfg(test)]
mod switching {
  use super::*;

  #[test]
  fn next_and_previous_wrap_while_a_number_names_a_tab() {
    assert_eq!(switched(3, Stop::Tab(2), TabMotion::Next), Ok(Some(0)));
    assert_eq!(
      switched(3, Stop::Tab(0), TabMotion::Previous(1)),
      Ok(Some(2))
    );
    assert_eq!(
      switched(3, Stop::Tab(0), TabMotion::Previous(6)),
      Ok(Some(0))
    );
    assert_eq!(switched(3, Stop::Tab(1), TabMotion::Number(3)), Ok(Some(2)));
    assert_eq!(switched(3, Stop::Tab(1), TabMotion::Number(4)), Ok(None));
    assert_eq!(switched(0, Stop::Outside, TabMotion::Next), Ok(None));
  }
}

pub fn along(tabs: u32, from: Stop, step: i32) -> Result<Stop, Never> {
  let Ok(out) = fitted::<u32, i32>(tabs);

  let at = match from {
    Stop::Tab(index) => {
      let Ok(index_2) = fitted::<u32, i32>(index);

      index_2.min(out)
    }
    Stop::Outside => out,
  };

  let going = at.saturating_add(step).clamp(0, out);

  Ok(match ::core::primitive::bool::then_some(going == out, ()) {
    ::core::option::Option::Some(()) => Stop::Outside,
    ::core::option::Option::None => {
      let Ok(going_2) = fitted(going);

      Stop::Tab(going_2)
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_strip_with_room_for_all_of_them_starts_at_the_first() {
    assert_eq!(
      visible_range(Tabs {
        many: 5,
        here: 3,
        from: 0,
        fits: 5
      }),
      Ok(0..5)
    );
    assert_eq!(
      visible_range(Tabs {
        many: 5,
        here: 3,
        from: 2,
        fits: 9
      }),
      Ok(0..5)
    );
  }

  #[test]
  fn the_run_moves_as_little_as_it_can() {
    let run = |here, from| {
      visible_range(Tabs {
        many: 5,
        here,
        from,
        fits: 3,
      })
    };

    assert_eq!(run(0, 0), Ok(0..3), "standing on the first");
    assert_eq!(run(2, 0), Ok(0..3), "the third is already showing");
    assert_eq!(run(3, 0), Ok(1..4), "one step, because it had to");
    assert_eq!(run(1, 2), Ok(1..4), "back the other way, one step");
  }

  #[test]
  fn the_run_never_hangs_off_either_end() {
    assert_eq!(
      visible_range(Tabs {
        many: 5,
        here: 4,
        from: 0,
        fits: 3
      }),
      Ok(2..5)
    );
    assert_eq!(
      visible_range(Tabs {
        many: 5,
        here: 4,
        from: 9,
        fits: 3
      }),
      Ok(2..5)
    );
  }

  #[test]
  fn a_tab_and_a_gap_is_what_a_tab_costs() {
    assert_eq!(
      fits(100, Cell(20)),
      Ok(4),
      "four tabs and the gaps between them"
    );
    assert_eq!(
      fits(0, Cell(20)),
      Ok(1),
      "somewhere to stand, whatever the room"
    );
  }

  #[test]
  fn the_room_is_the_card_less_everything_that_is_not_a_tab() {
    assert_eq!(
      room(Card {
        width: 900,
        spent: 120
      }),
      Ok(
        900_i32
          .saturating_sub(EDGE.saturating_mul(2))
          .saturating_sub(MARGIN.saturating_mul(2))
          .saturating_sub(PAD.saturating_mul(2))
          .saturating_sub(120)
      )
    );
  }

  #[test]
  fn a_shoulder_walks_the_tabs_and_then_the_way_out() {
    assert_eq!(along(3, Stop::Tab(0), 1), Ok(Stop::Tab(1)));
    assert_eq!(along(3, Stop::Tab(2), 1), Ok(Stop::Outside));
    assert_eq!(along(3, Stop::Outside, -1), Ok(Stop::Tab(2)));
  }

  #[test]
  fn the_walk_stops_at_both_ends() {
    assert_eq!(along(3, Stop::Tab(0), -1), Ok(Stop::Tab(0)));
    assert_eq!(along(3, Stop::Outside, 1), Ok(Stop::Outside));
  }

  #[test]
  fn the_way_out_is_there_on_a_panel_with_one_tab() {
    assert_eq!(along(1, Stop::Tab(0), 1), Ok(Stop::Outside));
    assert_eq!(along(1, Stop::Outside, -1), Ok(Stop::Tab(0)));
  }
}
