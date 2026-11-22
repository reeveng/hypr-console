use console_core_color::{Oklch, palette::Wearing};
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_shapes::{Covers, Edge, Panel, Picture, Round, Shape, Text, Weight};
use console_draw_painting::{self as painting, Frame, Lines, Run};
use console_draw_surface::{Surface, SurfaceError};

use super::Camera;

const MARGIN: u32 = 16;
const GAP: u32 = 8;
const ROW: u32 = 48;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
  Previous,
  Next,
  Take,
  Retry,
  Close,
}

impl Action {
  fn label(self) -> &'static str {
    match self {
      Self::Previous => "Previous",
      Self::Next => "Next",
      Self::Take => "Take Photo",
      Self::Retry => "Retry",
      Self::Close => "Close",
    }
  }
}

pub(super) struct Layout {
  pub points: Size<u32>,
  preview: Panel,
  heading: Panel,
  status: Panel,
  buttons: Vec<(Action, Panel, bool)>,
}

impl Layout {
  pub fn new(points: Size<u32>, camera: &Camera) -> Self {
    let wide = points.width.saturating_sub(MARGIN * 2);
    let bottom = points.height.saturating_sub(MARGIN + ROW);
    let top = MARGIN + ROW + GAP;
    let status_top = bottom.saturating_sub(26);
    let preview = bounds(MARGIN, top, wide, status_top.saturating_sub(top + GAP));
    let close_width = 76.min(wide);
    let heading = bounds(MARGIN, MARGIN, wide.saturating_sub(close_width + GAP), ROW);
    let close = bounds(
      MARGIN + wide.saturating_sub(close_width),
      MARGIN,
      close_width,
      ROW,
    );
    let status = bounds(MARGIN, status_top, wide, 24);
    let mut buttons = vec![(Action::Close, close, true)];

    let main = if camera.stream.is_some() {
      Action::Take
    } else {
      Action::Retry
    };
    let main_width = 176.min(wide.saturating_sub(GAP * 2) / 2);
    let main_left = MARGIN + wide.saturating_sub(main_width) / 2;
    let enabled = main == Action::Retry || camera.frame.is_some();
    buttons.push((main, bounds(main_left, bottom, main_width, ROW), enabled));

    if camera.sources.len() > 1 {
      let side_width = 100.min(main_left.saturating_sub(MARGIN + GAP));
      buttons.extend([
        (
          Action::Previous,
          bounds(
            main_left.saturating_sub(side_width + GAP),
            bottom,
            side_width,
            ROW,
          ),
          true,
        ),
        (
          Action::Next,
          bounds(main_left + main_width + GAP, bottom, side_width, ROW),
          true,
        ),
      ]);
    }

    Self {
      points,
      preview,
      heading,
      status,
      buttons,
    }
  }

  pub fn action(&self, at: (f64, f64)) -> Option<Action> {
    let point = Point {
      x: at.0.floor() as i32,
      y: at.1.floor() as i32,
    };
    self.buttons.iter().find_map(|(action, panel, enabled)| {
      let Ok(covers) = panel.covers(point);
      (*enabled && covers == Covers::Yes).then_some(*action)
    })
  }
}

pub(super) fn paint(
  surface: &mut Surface,
  layout: &Layout,
  camera: &Camera,
  wearing: &Wearing,
) -> Result<(), SurfaceError> {
  let points = layout.points;
  let background = [
    Shape::Panel(Panel {
      fill: wearing.ground,
      ..bounds(0, 0, points.width, points.height)
    }),
    Shape::Panel(Panel {
      fill: wearing.night,
      ..layout.preview
    }),
  ];
  let mut controls = Vec::new();
  if let Some(source) = camera.sources.get(camera.selected) {
    controls.push(label(
      layout.heading,
      &source.label,
      TextStyle::Headline,
      wearing.text,
      1,
    ));
  }
  for &(action, panel, enabled) in &layout.buttons {
    let primary = action == Action::Take || action == Action::Retry;
    controls.push(Shape::Panel(Panel {
      fill: wearing.panel,
      round: Round(8),
      edge: Edge::Of {
        wide: 1,
        color: if primary && enabled {
          wearing.pink
        } else {
          wearing.edge
        },
      },
      ..panel
    }));
    controls.push(label(
      panel,
      action.label(),
      TextStyle::Callout,
      if enabled { wearing.text } else { wearing.soft },
      1,
    ));
  }

  let picture = camera.frame.as_ref().and_then(|pixels| {
    let available = layout.preview.size;
    (available.width > 0 && available.height > 0).then(|| {
      let Ok(size) = console_pictures::fit_within(
        Size {
          width: pixels.width,
          height: pixels.height,
        },
        available,
      );
      let area = layout.preview;
      Picture {
        at: Point {
          x: area
            .at
            .x
            .saturating_add(coordinate(available.width.saturating_sub(size.width) / 2)),
          y: area
            .at
            .y
            .saturating_add(coordinate(available.height.saturating_sub(size.height) / 2)),
        },
        size,
        pixels,
      }
    })
  });
  if !camera.message.is_empty() {
    let (area, lines) = if picture.is_some() {
      (layout.status, 1)
    } else {
      (layout.preview, 3)
    };
    controls.push(label(
      area,
      &camera.message,
      TextStyle::Callout,
      wearing.text,
      lines,
    ));
  }

  surface.draw(|pixels, device, _scale| {
    let frame = Frame { points, device };
    let painted = painting::onto(pixels, frame, &background).and_then(|()| match picture {
      Some(picture) => painting::over_picture(pixels, frame, picture, &controls),
      None => painting::over(pixels, frame, &controls),
    });
    if let Err(fault) = painted {
      console_core_logging::warn!("drawing camera: {fault}");
    }
    Ok(())
  })
}

fn coordinate(value: u32) -> i32 {
  i32::try_from(value).unwrap_or(i32::MAX)
}

fn bounds(x: u32, y: u32, width: u32, height: u32) -> Panel {
  Panel {
    at: Point {
      x: coordinate(x),
      y: coordinate(y),
    },
    size: Size { width, height },
    round: Round(0),
    fill: Oklch::BLACK,
    edge: Edge::None,
  }
}

fn label(area: Panel, text: &str, style: TextStyle, ink: Oklch, lines: u32) -> Shape {
  let Ok(font) = style.font();
  let weight = Weight::Bold;
  let width = area.size.width.saturating_sub(12);
  let Ok(said) = painting::at_most(
    Run {
      said: text,
      weight,
      width,
    },
    &font,
    Lines(lines),
  );
  let Ok(measured) = painting::measure_text(
    Run {
      said: &said,
      weight,
      width,
    },
    &font,
  );
  Shape::Text(Text {
    at: Point {
      x: area.at.x.saturating_add(coordinate(
        area.size.width.saturating_sub(measured.width) / 2,
      )),
      y: area.at.y.saturating_add(coordinate(
        area.size.height.saturating_sub(measured.height) / 2,
      )),
    },
    width,
    said,
    weight,
    font,
    ink,
  })
}
