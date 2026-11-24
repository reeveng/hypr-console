use std::convert::Infallible;

use console_core_color::{Oklch, palette::Wearing};
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_shapes::{Edge, Panel, Picture, Pixels, Round, Shape, Text, Weight};
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
  fn label(self) -> Result<&'static str, Infallible> {
    Ok(match self {
      Self::Previous => "Previous",
      Self::Next => "Next",
      Self::Take => "Take Photo",
      Self::Retry => "Retry",
      Self::Close => "Close",
    })
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
  pub fn new(points: Size<u32>, camera: &Camera) -> Result<Self, Infallible> {
    let wide = points.width.saturating_sub(MARGIN.wrapping_mul(2));
    let bottom = points.height.saturating_sub(MARGIN.wrapping_add(ROW));
    let top = MARGIN.wrapping_add(ROW).wrapping_add(GAP);
    let status_top = bottom.saturating_sub(26);
    let Ok(preview) = bounds(
      Point { x: MARGIN, y: top },
      Size {
        width: wide,
        height: status_top.saturating_sub(top.wrapping_add(GAP)),
      },
    );
    let Ok((heading, close)) = title_row(wide);
    let Ok(status) = bounds(
      Point {
        x: MARGIN,
        y: status_top,
      },
      Size {
        width: wide,
        height: 24,
      },
    );
    let source_switching = camera.sources.len() > 1;
    let mut buttons = match source_switching.then_some(()) {
      Some(()) => Vec::with_capacity(4),
      None => Vec::with_capacity(2),
    };
    buttons.push((Action::Close, close, true));

    let main = match &camera.stream {
      Some(_stream) => Action::Take,
      None => Action::Retry,
    };
    let main_width = 176.min(wide.saturating_sub(GAP.wrapping_mul(2)).wrapping_div(2));
    let main_left = MARGIN.wrapping_add(wide.saturating_sub(main_width).wrapping_div(2));
    let enabled = main == Action::Retry || camera.frame.is_some();
    let Ok(main_button) = bounds(
      Point {
        x: main_left,
        y: bottom,
      },
      Size {
        width: main_width,
        height: ROW,
      },
    );
    buttons.push((main, main_button, enabled));

    match source_switching.then_some(()) {
      Some(()) => {
        let side_width = 100.min(main_left.saturating_sub(MARGIN.wrapping_add(GAP)));
        let side = Size {
          width: side_width,
          height: ROW,
        };
        let Ok(previous) = bounds(
          Point {
            x: main_left.saturating_sub(side_width.wrapping_add(GAP)),
            y: bottom,
          },
          side,
        );
        let Ok(next) = bounds(
          Point {
            x: main_left.wrapping_add(main_width).wrapping_add(GAP),
            y: bottom,
          },
          side,
        );
        buttons.extend([
          (Action::Previous, previous, true),
          (Action::Next, next, true),
        ]);
      }
      None => {}
    }

    Ok(Self {
      points,
      preview,
      heading,
      status,
      buttons,
    })
  }

  pub fn action(&self, at: (f64, f64)) -> Result<Option<Action>, Infallible> {
    let [x, y] = [at.0, at.1].map(|value| match value.is_nan().then_some(()) {
      Some(()) => 0.0,
      None => value,
    });
    Ok(self.buttons.iter().find_map(|(action, panel, enabled)| {
      let Ok(width) = coordinate(panel.size.width);
      let Ok(height) = coordinate(panel.size.height);
      let left = f64::from(panel.at.x);
      let top = f64::from(panel.at.y);
      let right = f64::from(panel.at.x.saturating_add(width));
      let bottom = f64::from(panel.at.y.saturating_add(height));
      (*enabled && (left..right).contains(&x) && (top..bottom).contains(&y)).then_some(*action)
    }))
  }
}

fn title_row(wide: u32) -> Result<(Panel, Panel), Infallible> {
  let close_width = 76.min(wide);
  let Ok(heading) = bounds(
    Point {
      x: MARGIN,
      y: MARGIN,
    },
    Size {
      width: wide.saturating_sub(close_width.wrapping_add(GAP)),
      height: ROW,
    },
  );
  let Ok(close) = bounds(
    Point {
      x: MARGIN.wrapping_add(wide.saturating_sub(close_width)),
      y: MARGIN,
    },
    Size {
      width: close_width,
      height: ROW,
    },
  );
  Ok((heading, close))
}

pub(super) fn paint(
  surface: &mut Surface,
  layout: &Layout,
  camera: &Camera,
  wearing: &Wearing,
) -> Result<(), SurfaceError> {
  let points = layout.points;
  let Ok(window) = bounds(Point { x: 0, y: 0 }, points);
  let background = [
    Shape::Panel(Panel {
      fill: wearing.ground,
      ..window
    }),
    Shape::Panel(Panel {
      fill: wearing.night,
      ..layout.preview
    }),
  ];
  let mut controls = Vec::new();

  let Ok(selected) = camera.selected_source();

  match selected {
    Some(source) => {
      let Ok(heading) = label(
        layout.heading,
        &source.label,
        TextStyle::Headline,
        wearing.text,
        1,
      );
      controls.push(heading);
    }
    ::core::option::Option::None => {}
  }

  for &(action, panel, enabled) in &layout.buttons {
    let primary = action == Action::Take || action == Action::Retry;
    controls.push(Shape::Panel(Panel {
      fill: wearing.panel,
      round: Round(8),
      edge: Edge::Of {
        wide: 1,
        color: match { ::core::primitive::bool::then_some(primary && enabled, ()) } {
          ::core::option::Option::Some(()) => wearing.pink,
          ::core::option::Option::None => wearing.edge,
        },
      },
      ..panel
    }));
    let Ok(title) = action.label();
    let Ok(text) = label(
      panel,
      title,
      TextStyle::Callout,
      match ::core::primitive::bool::then_some(enabled, ()) {
        ::core::option::Option::Some(()) => wearing.text,
        ::core::option::Option::None => wearing.soft,
      },
      1,
    );
    controls.push(text);
  }

  let Ok(picture) = preview_picture(layout.preview, camera.frame.as_ref());

  match { ::core::primitive::bool::then_some(!camera.message.is_empty(), ()) } {
    ::core::option::Option::Some(()) => {
      let (area, lines) = match picture {
        ::core::option::Option::Some(_) => (layout.status, 1),
        ::core::option::Option::None => (layout.preview, 3),
      };
      let Ok(message) = label(
        area,
        &camera.message,
        TextStyle::Callout,
        wearing.text,
        lines,
      );
      controls.push(message);
    }
    ::core::option::Option::None => {}
  }

  surface.draw(|pixels, device, _scale| {
    let frame = Frame { points, device };
    let painted = match painting::onto(pixels, frame, &background) {
      Ok(()) => match picture {
        Some(preview) => painting::over_picture(pixels, frame, preview, &controls),
        None => painting::over(pixels, frame, &controls),
      },
      Err(fault) => Err(fault),
    };

    match painted {
      Err(fault) => {
        console_core_logging::warn!("drawing camera: {fault}");
      }
      ::core::result::Result::Ok(_) => {}
    }

    Ok(())
  })
}

fn preview_picture(
  area: Panel,
  pixels: Option<&Pixels>,
) -> Result<Option<Picture<&Pixels>>, Infallible> {
  let picture = pixels.and_then(|frame| {
    let available = area.size;
    (available.width > 0 && available.height > 0).then(|| {
      let Ok(size) = console_pictures::fit_within(
        Size {
          width: frame.width,
          height: frame.height,
        },
        available,
      );
      let Ok(at) = centered(area, size);
      Picture {
        at,
        size,
        pixels: frame,
      }
    })
  });
  Ok(picture)
}

fn centered(area: Panel, size: Size<u32>) -> Result<Point<i32>, Infallible> {
  let Ok(x) = coordinate(area.size.width.saturating_sub(size.width).wrapping_div(2));
  let Ok(y) = coordinate(area.size.height.saturating_sub(size.height).wrapping_div(2));
  Ok(Point {
    x: area.at.x.saturating_add(x),
    y: area.at.y.saturating_add(y),
  })
}

fn coordinate(value: u32) -> Result<i32, Infallible> {
  Ok(match i32::try_from(value) {
    Ok(coordinate) => coordinate,
    Err(_coordinate_exceeds_signed_range) => i32::MAX,
  })
}

fn bounds(origin: Point<u32>, size: Size<u32>) -> Result<Panel, Infallible> {
  let Ok(x) = coordinate(origin.x);
  let Ok(y) = coordinate(origin.y);
  Ok(Panel {
    at: Point { x, y },
    size,
    round: Round(0),
    fill: Oklch::BLACK,
    edge: Edge::None,
  })
}

fn label(
  area: Panel,
  text: &str,
  style: TextStyle,
  ink: Oklch,
  lines: u32,
) -> Result<Shape, Infallible> {
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
  let Ok(at) = centered(area, measured);
  Ok(Shape::Text(Text {
    at,
    width,
    said,
    weight,
    font,
    ink,
  }))
}
