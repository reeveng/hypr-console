use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_shapes::{Covers, Edge, Line, Panel, Round, Shape};

use crate::{BUTTON_TALL, MARGIN, label};

pub const CANCEL: &str = "Cancel";
pub const CORNER_CONTROLS: &str =
  "Arrows / D-pad move · Enter / A picks\nTab / Y toggles fine · Esc / B cancels";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
  Text,
  Screenshot,
}

impl Purpose {
  pub fn instruction(self) -> &'static str {
    match self {
      Self::Text => "Drag over the text",
      Self::Screenshot => "Drag over an area",
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub struct Prompt {
  pub instruction: Panel,
  pub cancel: Panel,
  pub controls: Panel,
}

pub fn prompt(room: Size<u32>, wearing: &Wearing) -> Result<Prompt, Never> {
  const CANCEL_WIDTH: u32 = 88;
  const GAP: u32 = 8;
  let wide = room.width.saturating_sub(MARGIN.saturating_mul(2)).min(320);
  let instruction = wide.saturating_sub(CANCEL_WIDTH.saturating_add(GAP));
  let Ok(left) = fitted::<u32, i32>(room.width.saturating_sub(wide).saturating_div(2));
  let Ok(top) = fitted::<u32, i32>(MARGIN);
  let Ok(step) = fitted::<u32, i32>(instruction.saturating_add(GAP));
  let controls_wide = room.width.saturating_sub(MARGIN.saturating_mul(2)).min(420);
  let Ok(controls_left) = fitted::<u32, i32>(room.width.saturating_sub(controls_wide) / 2);
  let Ok(controls_top) = fitted::<u32, i32>(room.height.saturating_sub(MARGIN + BUTTON_TALL));

  let panel = |x, width| Panel {
    at: Point { x, y: top },
    size: Size {
      width,
      height: BUTTON_TALL,
    },
    round: Round(crate::BUTTON_ROUND),
    fill: wearing.panel,
    edge: Edge::Of {
      wide: 2,
      color: wearing.edge,
    },
  };

  Ok(Prompt {
    instruction: panel(left, instruction),
    cancel: panel(left.saturating_add(step), CANCEL_WIDTH.min(wide)),
    controls: Panel {
      at: Point {
        x: controls_left,
        y: controls_top,
      },
      ..panel(controls_left, controls_wide)
    },
  })
}

impl Prompt {
  pub fn cancels(self, at: Point<f64>) -> Result<Covers, Never> {
    let Ok(x) = toward_zero_i32(at.x.floor());
    let Ok(y) = toward_zero_i32(at.y.floor());
    self.cancel.covers(Point { x, y })
  }

  pub fn shapes(
    self,
    said: &str,
    instruction: Size<u32>,
    cancel: Size<u32>,
    wearing: &Wearing,
  ) -> Result<[Shape; 4], Never> {
    let Ok(hint) = label(self.instruction, instruction, said, wearing);
    let Ok(cancel_label) = label(self.cancel, cancel, CANCEL, wearing);
    Ok([
      Shape::Panel(self.instruction),
      hint,
      Shape::Panel(self.cancel),
      cancel_label,
    ])
  }

  pub fn control_shapes(
    self,
    measured: Size<u32>,
    wearing: &Wearing,
  ) -> Result<[Shape; 2], Never> {
    let Ok(words) = label(self.controls, measured, CORNER_CONTROLS, wearing);
    Ok([Shape::Panel(self.controls), words])
  }
}

#[derive(Clone, Copy)]
pub struct Corners {
  at: Point<i32>,
  first: Option<Point<i32>>,
  fine: bool,
}

impl Corners {
  pub fn centered(room: Size<u32>) -> Self {
    let Ok(x) = fitted::<u32, i32>(room.width / 2);
    let Ok(y) = fitted::<u32, i32>(room.height / 2);
    Self {
      at: Point { x, y },
      first: None,
      fine: false,
    }
  }

  pub fn move_by(&mut self, dx: i32, dy: i32, room: Size<u32>) {
    let step = if self.fine { 1 } else { 32 };
    let Ok(right) = fitted::<u32, i32>(room.width);
    let Ok(bottom) = fitted::<u32, i32>(room.height);
    self.at.x = self
      .at
      .x
      .saturating_add(dx.saturating_mul(step))
      .clamp(0, right);
    self.at.y = self
      .at
      .y
      .saturating_add(dy.saturating_mul(step))
      .clamp(0, bottom);
  }

  pub fn toggle_fine(&mut self) {
    self.fine = !self.fine;
  }

  pub fn pick(&mut self, room: Size<u32>) -> Result<Option<Rectangle<i32>>, Never> {
    match self.first {
      Some(_) => self.region(room),
      None => {
        self.first = Some(self.at);
        Ok(None)
      }
    }
  }

  fn region(self, room: Size<u32>) -> Result<Option<Rectangle<i32>>, Never> {
    match self.first {
      Some(first) => {
        let Ok(from) = first.map(f64::from);
        let Ok(at) = self.at.map(f64::from);
        Drag { from, at }.region(room)
      }
      None => Ok(None),
    }
  }

  pub fn instruction(self) -> &'static str {
    match (self.first, self.fine) {
      (None, false) => "Pick first corner",
      (None, true) => "Pick first corner · fine",
      (Some(_), false) => "Pick opposite corner",
      (Some(_), true) => "Pick opposite corner · fine",
    }
  }

  pub fn shapes(self, room: Size<u32>, wearing: &Wearing) -> Result<Vec<Shape>, Never> {
    let Ok(region) = self.region(room);
    let mut shapes = Vec::new();
    if let Some(region) = region {
      let Ok(lines) = outline(region, wearing);
      shapes.extend(lines);
    }
    if let Some(first) = self.first.filter(|first| *first != self.at) {
      shapes.extend(crosshair(first, 5, wearing));
    }
    shapes.extend(crosshair(self.at, 10, wearing));
    Ok(shapes)
  }
}

fn crosshair(at: Point<i32>, reach: i32, wearing: &Wearing) -> [Shape; 2] {
  let Point { x, y } = at;
  [
    (
      Point {
        x: x.saturating_sub(reach),
        y,
      },
      Point {
        x: x.saturating_add(reach),
        y,
      },
    ),
    (
      Point {
        x,
        y: y.saturating_sub(reach),
      },
      Point {
        x,
        y: y.saturating_add(reach),
      },
    ),
  ]
  .map(|(from, to)| {
    Shape::Line(Line {
      from,
      to,
      width: 2,
      color: wearing.pink,
    })
  })
}

#[derive(Clone, Copy)]
pub struct Drag {
  pub from: Point<f64>,
  pub at: Point<f64>,
}

impl Drag {
  pub fn region(self, room: Size<u32>) -> Result<Option<Rectangle<i32>>, Never> {
    let right = f64::from(room.width);
    let bottom = f64::from(room.height);
    let Ok(x) = toward_zero_i32(self.from.x.min(self.at.x).floor().clamp(0.0, right));
    let Ok(y) = toward_zero_i32(self.from.y.min(self.at.y).floor().clamp(0.0, bottom));
    let Ok(end_x) = toward_zero_i32(self.from.x.max(self.at.x).ceil().clamp(0.0, right));
    let Ok(end_y) = toward_zero_i32(self.from.y.max(self.at.y).ceil().clamp(0.0, bottom));
    let width = end_x.saturating_sub(x);
    let height = end_y.saturating_sub(y);

    Ok(
      match ::core::primitive::bool::then_some(width >= 4 && height >= 4, ()) {
        ::core::option::Option::Some(()) => Some(Rectangle {
          origin: Point { x, y },
          size: Size { width, height },
        }),
        ::core::option::Option::None => None,
      },
    )
  }
}

pub fn outline(region: Rectangle<i32>, wearing: &Wearing) -> Result<[Shape; 4], Never> {
  let Point { x, y } = region.origin;
  let right = x.saturating_add(region.size.width);
  let bottom = y.saturating_add(region.size.height);
  let top_left = region.origin;
  let top_right = Point { x: right, y };
  let bottom_right = Point {
    x: right,
    y: bottom,
  };
  let bottom_left = Point { x, y: bottom };

  Ok(
    [
      (top_left, top_right),
      (top_right, bottom_right),
      (bottom_right, bottom_left),
      (bottom_left, top_left),
    ]
    .map(|(from, to)| {
      Shape::Line(Line {
        from,
        to,
        width: 2,
        color: wearing.pink,
      })
    }),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_reverse_drag_is_clipped_to_the_surface_and_rounded_outward() {
    let room = Size {
      width: 100,
      height: 70,
    };
    let from = Point { x: 103.0, y: 52.25 };
    let at = Point { x: -4.0, y: 20.75 };
    let expected = Some(Rectangle {
      origin: Point { x: 0, y: 20 },
      size: Size {
        width: 100,
        height: 33,
      },
    });

    assert_eq!(Drag { from, at }.region(room), Ok(expected));
    assert_eq!(Drag { from: at, at: from }.region(room), Ok(expected));
    assert_eq!(Drag { from, at: from }.region(room), Ok(None));
  }
}
