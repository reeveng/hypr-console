use std::convert::Infallible;

use console_core_color::{Oklch, palette::Wearing};
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_panel::ui::canvas::{self, Area, AreaLayout, Canvas, Interaction, Style};
use console_panel::ui::painting::{self, Frame, Lines, Run};
use console_panel::ui::shapes::{
  Edge, Panel, Picture, Pixels, Round, Shape, Text, Weight, hover_indicator,
};
use console_panel::ui::strip::{self, Button, Header};
use console_panel::ui::surface::{Surface, SurfaceError};
use console_panel::ui::theme;

use super::Camera;

const MARGIN: u32 = strip::WINDOW_INSET.unsigned_abs();
const GAP: u32 = strip::GAP.unsigned_abs();
const ROW: u32 = strip::HEIGHT.unsigned_abs();

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
  Source(u32),
  Previous,
  Next,
  Take,
  Retry,
  Close,
}

impl Action {
  fn label(self) -> Result<&'static str, Infallible> {
    Ok(match self {
      Self::Take => "Take Photo",
      Self::Retry => "Retry",
      Self::Source(_) | Self::Previous | Self::Next | Self::Close => "",
    })
  }
}

pub(super) struct Layout {
  pub points: Size<u32>,
  preview: Panel,
  header: canvas::Frame<Action>,
  status: Panel,
  capture: Area,
}

impl Layout {
  pub fn new(points: Size<u32>, camera: &Camera, colors: &Wearing) -> Result<Self, Infallible> {
    let wide = points.width.saturating_sub(MARGIN.wrapping_mul(2));
    let bottom = points.height.saturating_sub(MARGIN.wrapping_add(ROW));
    let top = strip::CONTENT_TOP.unsigned_abs();
    let status_top = bottom.saturating_sub(26);
    let Ok(preview) = bounds(
      Point { x: MARGIN, y: top },
      Size {
        width: wide,
        height: status_top.saturating_sub(top.wrapping_add(GAP)),
      },
    );
    let Ok(header) = header(points, camera, colors);
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
    let main_width = 176.min(wide.saturating_sub(GAP.wrapping_mul(2)).wrapping_div(2));
    let main_left = MARGIN.wrapping_add(wide.saturating_sub(main_width).wrapping_div(2));
    let Ok(x) = coordinate(main_left);
    let Ok(y) = coordinate(bottom);
    let Ok(capture_width) = coordinate(main_width);
    let main_button = Area {
      origin: Point { x, y },
      size: Size {
        width: capture_width,
        height: strip::HEIGHT,
      },
    };
    Ok(Self {
      points,
      preview,
      header,
      status,
      capture: main_button,
    })
  }

  pub fn action(&self, at: (f64, f64), camera: &Camera) -> Result<Option<Action>, Infallible> {
    let Ok(target) = self.hit(Point::from(at), camera);

    Ok(target.map(|(_, action)| action))
  }

  pub fn hit(
    &self,
    at: Point<f64>,
    camera: &Camera,
  ) -> Result<Option<(Area, Action)>, Infallible> {
    let Ok(capture) = capture_action(camera);
    let area = self.capture;
    let Ok(inside) = area.contains_point(at);

    match inside {
      console_core_geometry::Containment::Inside => Ok(capture.map(|action| (area, action))),
      console_core_geometry::Containment::Outside => {
        let Ok(hit) = self.header.targets.hit(at);

        Ok(hit)
      }
    }
  }
}

pub(super) fn capture_action(camera: &Camera) -> Result<Option<Action>, Infallible> {
  let Ok(button) = capture_button(camera);
  let Ok(action) = button.action();
  Ok(action)
}

fn capture_button(camera: &Camera) -> Result<Button<Action>, Infallible> {
  Ok(match (&camera.stream, &camera.frame) {
    (None, _) => Button::Primary(Action::Retry),
    (Some(_), Some(_)) => Button::Primary(Action::Take),
    (Some(_), None) => Button::Loading,
  })
}

fn header(
  points: Size<u32>,
  camera: &Camera,
  colors: &Wearing,
) -> Result<canvas::Frame<Action>, Infallible> {
  let Ok(width) = coordinate(points.width);
  let Ok(height) = coordinate(points.height);
  let root = Area {
    origin: Point { x: 0, y: 0 },
    size: Size { width, height },
  };
  let Ok(mut canvas) = Canvas::new(
    root,
    Style::from(colors),
    Interaction {
      focus: None,
      hover: None,
    },
  );
  let labels: Vec<_> = camera
    .sources
    .iter()
    .map(|source| source.label.as_str())
    .collect();
  let tabs = match labels.as_slice() {
    [] | [_] => &[],
    available => available,
  };
  let Ok(area) = strip::window_area(width);
  let Ok(()) = strip::header(
    &mut canvas,
    area,
    &Header {
      title: "Camera",
      tabs,
      selected: camera.selected,
      select: Action::Source,
      previous: Action::Previous,
      next: Action::Next,
      close: Action::Close,
    },
    colors,
  );
  Ok(canvas.frame)
}

pub(super) fn paint(
  surface: &mut Surface,
  layout: &Layout,
  camera: &Camera,
  (wearing, hover): (&Wearing, Option<Area>),
) -> Result<(), SurfaceError> {
  let points = layout.points;
  let Ok(window) = bounds(Point { x: 0, y: 0 }, points);
  let Ok(ground) = theme::Surface::Window.color(wearing);
  let Ok(recess) = theme::Surface::Recessed.color(wearing);
  let background = [
    Shape::Panel(Panel {
      fill: ground,
      ..window
    }),
    Shape::Panel(Panel {
      fill: recess,
      ..layout.preview
    }),
  ];
  let Ok(button) = capture_button(camera);
  let Ok(action) = button.action();
  let mut controls = Vec::new();
  let Ok(state) = button.face(None);
  let Ok(title) = match action {
    Some(available) => available.label(),
    None => Ok("Starting camera…"),
  };
  let Ok(()) = strip::paint_control(&mut controls, layout.capture, title, state, wearing);

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

  match hover {
    Some(area) => {
      let Ok(size) = area.pixel_size();
      let Ok(indicator) = hover_indicator(area.origin, size, (wearing.pink, wearing.night));
      controls.extend(indicator);
    }
    None => {}
  }

  surface.draw(|pixels, device, _scale| {
    let frame = Frame { points, device };
    let painted = match painting::onto(pixels, frame, &background) {
      Ok(()) => match picture {
        Some(preview) => painting::over_picture(pixels, frame, preview, &layout.header.shapes),
        None => painting::over(pixels, frame, &layout.header.shapes),
      },
      Err(fault) => Err(fault),
    };

    let painted_controls = match painted {
      Ok(()) => painting::over(pixels, frame, &controls),
      Err(fault) => Err(fault),
    };

    match painted_controls {
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
      let Ok(picture) = Picture::rectangular(at, size, frame);
      picture
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::source::Source;
  use std::path::PathBuf;

  #[test]
  fn sources_are_in_the_header_and_capture_stays_below_the_preview()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(palette) = console_core_color::palette::read(include_str!(
      "../../../../../files/usr/local/lib/console/palette.sh"
    ));
    let colors = Wearing::out_of(&palette)?;
    let camera = Camera {
      sources: vec![
        Source {
          path: PathBuf::from("/dev/video0"),
          label: "Front".to_owned(),
        },
        Source {
          path: PathBuf::from("/dev/video1"),
          label: "Rear".to_owned(),
        },
      ],
      selected: 0,
      stream: None,
      frame: None,
      message: String::new(),
    };
    let Ok(layout) = Layout::new(
      Size {
        width: 780,
        height: 560,
      },
      &camera,
      &colors,
    );

    for mark in [
      console_panel::marks::BEFORE,
      console_panel::marks::AFTER,
      console_panel::marks::SHUT,
    ] {
      assert!(
        layout
          .header
          .shapes
          .iter()
          .any(|shape| matches!(shape, Shape::Text(label) if label.said == mark))
      );
    }

    assert!(matches!(layout.action((32.0, 32.0), &camera), Ok(None)));
    assert!(matches!(
      layout.action((520.0, 32.0), &camera),
      Ok(Some(Action::Source(1)))
    ));
    assert!(matches!(
      layout.action((690.0, 32.0), &camera),
      Ok(Some(Action::Next))
    ));
    assert!(matches!(
      layout.action((745.0, 32.0), &camera),
      Ok(Some(Action::Close))
    ));
    assert!(matches!(
      layout.action((390.0, 520.0), &camera),
      Ok(Some(Action::Retry))
    ));
    Ok(())
  }
}
