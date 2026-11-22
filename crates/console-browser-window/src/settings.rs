//! Settings are drawn over the page and take input until dismissed.

use console_core_color::palette::Wearing;
use console_core_fonts::Font;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Actions, Heading, Press, Resolution, under};
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Shape};
use console_draw_painting::{self as drawing, Frame};
use console_input_key_symbols::Keysym;

use crate::reader::{Mode, TextSize};
use crate::renderer::Renderer;
use crate::toolbar::{Action, CONTROL, HEIGHT, SHIFT, Status, measured, panel, words};

const ROW: i32 = 32;
const GAP: i32 = 4;
const INSET: i32 = 12;
const MAX_WIDTH: i32 = 320;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
  Engines,
  Engine(Renderer),
  Reader(Option<Mode>),
  Text(TextSize),
}

#[derive(Clone, Copy, Default)]
enum View {
  #[default]
  Closed,
  Settings,
  EngineList,
}

#[derive(Default)]
pub(crate) struct Settings {
  view: View,
  focus: Option<u32>,
  bounds: Option<Rectangle<i32>>,
  frames: Vec<Rectangle<i32>>,
  controls: Vec<Control>,
}

impl Settings {
  pub(crate) fn visible(&self) -> bool {
    !matches!(self.view, View::Closed)
  }

  pub(crate) fn open(&mut self) {
    self.view = View::Settings;
    self.focus = Some(0);
  }

  fn close(&mut self) -> Action {
    self.view = View::Closed;
    self.focus = None;
    Action::SettingsChanged
  }

  pub(crate) fn tap(&mut self, point: Point<i32>, status: &Status) -> Action {
    match self.resolve(Press::Tap(point)) {
      Resolution::Activate { target, action } => {
        self.focus = Some(target);
        self.activate(action, status)
      }
      Resolution::Focus(_) | Resolution::Pass => {
        let inside = self.bounds.and_then(|frame| {
          let Ok(over) = under(point, &[frame]);
          over
        });
        match inside {
          Some(_) => Action::Handled,
          None => self.close(),
        }
      }
    }
  }

  pub(crate) fn key(&mut self, key: Keysym, mods: u32, status: &Status) -> Action {
    match (mods, key) {
      (0, Keysym::Escape) => match self.view {
        View::EngineList => {
          self.open();
          Action::SettingsChanged
        }
        View::Closed | View::Settings => self.close(),
      },
      (0, Keysym::F6 | Keysym::F18 | Keysym::XF86Launch9) => self.close(),
      (0, Keysym::F9) => self.activate(Control::Reader(Some(Mode::Dark)), status),
      (SHIFT, Keysym::F9) => self.activate(Control::Reader(Some(Mode::Desktop)), status),
      (CONTROL, Keysym::minus | Keysym::equal) => match status.reader {
        Some(Mode::Dark) => match key {
          Keysym::minus => Action::ReaderText(TextSize::Smaller),
          _ => Action::ReaderText(TextSize::Larger),
        },
        None | Some(Mode::Desktop) => Action::Handled,
      },
      (0, Keysym::Return | Keysym::KP_Enter | Keysym::space) => {
        match self.resolve(Press::Select) {
          Resolution::Activate { action, .. } => self.activate(action, status),
          Resolution::Focus(_) | Resolution::Pass => Action::Handled,
        }
      }
      _ => {
        let heading = match (mods, key) {
          (0, Keysym::Up) => Heading::Up,
          (0, Keysym::Down) => Heading::Down,
          (0, Keysym::Left) => Heading::Left,
          (0, Keysym::Right) => Heading::Right,
          (0, Keysym::Tab) => Heading::Next,
          (SHIFT, Keysym::Tab | Keysym::ISO_Left_Tab) => Heading::Previous,
          _ => return Action::Handled,
        };
        match self.resolve(Press::Toward(heading)) {
          Resolution::Focus(target) => self.focus = Some(target),
          Resolution::Activate { .. } | Resolution::Pass => {}
        }
        Action::Handled
      }
    }
  }

  fn resolve(&self, press: Press) -> Resolution<Control> {
    match Actions::new(&self.frames, &self.controls) {
      Ok(actions) => {
        let Ok(resolution) = actions.resolve(press, self.focus);
        resolution
      }
      Err(fault) => {
        console_core_logging::error!(
          "browser settings layout does not match its controls: {fault:?}"
        );
        Resolution::Pass
      }
    }
  }

  fn activate(&mut self, control: Control, status: &Status) -> Action {
    match control {
      Control::Engines => {
        match self.view {
          View::EngineList => self.open(),
          View::Settings | View::Closed => {
            self.view = View::EngineList;
            self.focus = Some(1);
          }
        }
        Action::SettingsChanged
      }
      Control::Engine(renderer) => {
        self.open();
        Action::Switch(renderer)
      }
      Control::Reader(mode) => match (status.reader == mode).then_some(()) {
        Some(()) => Action::Handled,
        None => match mode {
          Some(mode) => Action::Reader(mode),
          None => Action::PageView,
        },
      },
      Control::Text(size) => Action::ReaderText(size),
    }
  }

  pub(crate) fn paint(
    &mut self,
    pixels: &mut [u8],
    frame: Frame,
    palette: &Wearing,
    font: &Font,
    status: &Status,
    renderers: &[Renderer],
  ) -> Result<(), drawing::Cannot> {
    let choices = match self.view {
      View::Closed => return Ok(()),
      View::Settings => &[][..],
      View::EngineList => renderers,
    };
    self.frames.clear();
    self.controls.clear();
    let Ok(width) = fitted::<_, i32>(frame.points.width);
    let Ok(height) = fitted::<_, i32>(frame.points.height);
    let wide = width.saturating_sub(16).clamp(1, MAX_WIDTH);
    let Ok(choice_rows) = fitted::<_, i32>(choices.len());
    let text_rows = match status.reader {
      Some(Mode::Dark) => 1,
      None | Some(Mode::Desktop) => 0,
    };
    let rows = choice_rows.saturating_add(text_rows).saturating_add(3);
    let tall = rows
      .saturating_mul(ROW.saturating_add(GAP))
      .saturating_add(2 * INSET);
    let Ok(chrome_height) = fitted::<_, i32>(HEIGHT);
    let top = chrome_height
      .saturating_add(6)
      .min(height.saturating_sub(tall + 8))
      .max(50);
    let left = width.saturating_sub(wide).saturating_sub(8).max(0);
    let bounds = rectangle(left, top, wide, tall);
    self.bounds = Some(bounds);
    let Ok(backdrop) = panel(
      bounds,
      10,
      palette.panel,
      Edge::Of {
        wide: 1,
        color: palette.edge,
      },
    );
    let mut shapes = vec![backdrop];
    let content = wide.saturating_sub(2 * INSET).max(1);
    let x = left.saturating_add(INSET);
    let mut y = top.saturating_add(INSET);
    let row = |y| rectangle(x, y, content, ROW);
    let Ok(engine) = status.renderer.label();
    let arrow = match self.view {
      View::EngineList => "▴",
      View::Settings | View::Closed => "▾",
    };
    let label = format!("Engine   {engine} {arrow}");
    self.add(Control::Engines, row(y));
    y = y.saturating_add(ROW + GAP);
    for renderer in choices {
      self.add(Control::Engine(*renderer), row(y));
      y = y.saturating_add(ROW + GAP);
    }
    let reader_heading = rectangle(x, y, content, ROW);
    y = y.saturating_add(ROW + GAP);
    let segment = content.saturating_sub(2 * GAP).wrapping_div(3).max(1);
    for (offset, reader) in [
      (0i32, None),
      (1, Some(Mode::Dark)),
      (2, Some(Mode::Desktop)),
    ] {
      self.add(
        Control::Reader(reader),
        rectangle(
          x.saturating_add(offset.saturating_mul(segment + GAP)),
          y,
          segment,
          ROW,
        ),
      );
    }
    label_at(
      &mut shapes,
      reader_heading,
      "Reader",
      font,
      palette.text,
      Alignment::Start,
    );
    match status.reader {
      Some(Mode::Dark) => {
        y = y.saturating_add(ROW + GAP);
        let text_controls = content.saturating_sub(80).max(0);
        label_at(
          &mut shapes,
          rectangle(x, y, text_controls, ROW),
          "Text size   Ctrl+− / Ctrl+=",
          font,
          palette.text,
          Alignment::Start,
        );
        let buttons = x.saturating_add(text_controls);
        self.add(
          Control::Text(TextSize::Smaller),
          rectangle(buttons, y, 36, ROW),
        );
        self.add(
          Control::Text(TextSize::Larger),
          rectangle(buttons.saturating_add(40), y, 36, ROW),
        );
      }
      None | Some(Mode::Desktop) => {}
    }
    for (index, (area, control)) in self.frames.iter().zip(&self.controls).enumerate() {
      let Ok(index) = fitted(index);
      let (text, shortcut, alignment, active) = match control {
        Control::Engines => (label.as_str(), None, Alignment::Start, None),
        Control::Engine(renderer) => {
          let Ok(name) = renderer.label();
          (
            name,
            None,
            Alignment::Start,
            (*renderer == status.renderer).then_some(()),
          )
        }
        Control::Reader(reader) => {
          let (text, shortcut) = match reader {
            None => ("Page", None),
            Some(Mode::Dark) => ("Dark", Some("F9")),
            Some(Mode::Desktop) => ("Desktop", Some("Shift+F9")),
          };
          (
            text,
            shortcut,
            Alignment::Center,
            (*reader == status.reader).then_some(()),
          )
        }
        Control::Text(size) => {
          let text = match size {
            TextSize::Smaller => "A−",
            TextSize::Larger => "A+",
          };
          (text, None, Alignment::Center, None)
        }
      };
      let edge = match (self.focus == Some(index)).then_some(()).or(active) {
        Some(()) => Edge::Of {
          wide: 1,
          color: palette.pink,
        },
        None => Edge::None,
      };
      let fill = match active {
        Some(()) => palette.night,
        None => palette.fill,
      };
      let Ok(button) = panel(*area, 6, fill, edge);
      shapes.push(button);
      match shortcut {
        Some(keys) => {
          let caption = Font {
            height: 11,
            family: font.family.clone(),
          };
          let heading = rectangle(area.origin.x, area.origin.y, area.size.width, 19);
          let hint = rectangle(
            area.origin.x,
            area.origin.y.saturating_add(17),
            area.size.width,
            14,
          );
          label_at(&mut shapes, heading, text, font, palette.text, alignment);
          label_at(&mut shapes, hint, keys, &caption, palette.soft, alignment);
        }
        None => label_at(&mut shapes, *area, text, font, palette.text, alignment),
      }
    }
    drawing::over(pixels, frame, &shapes)
  }

  fn add(&mut self, control: Control, frame: Rectangle<i32>) {
    self.controls.push(control);
    self.frames.push(frame);
  }
}

#[derive(Clone, Copy)]
pub(crate) enum Alignment {
  Start,
  Center,
}

pub(crate) fn label_at(
  shapes: &mut Vec<Shape>,
  frame: Rectangle<i32>,
  label: &str,
  font: &Font,
  color: console_core_color::Oklch,
  alignment: Alignment,
) {
  let width = frame.size.width.saturating_sub(12).max(0).unsigned_abs();
  match width {
    0 => return,
    _ => {}
  }
  let Ok(text) = drawing::at_most(
    drawing::Run {
      said: label,
      weight: console_core_fonts::Weight::Plain,
      width,
    },
    font,
    drawing::Lines(1),
  );
  let Ok(size) = measured(&text, font);
  let Ok(tall) = fitted::<_, i32>(size.height);
  let Ok(wide) = fitted::<_, i32>(size.width);
  let offset = match alignment {
    Alignment::Start => 6,
    Alignment::Center => frame.size.width.saturating_sub(wide).wrapping_div(2).max(0),
  };
  let at = Point {
    x: frame.origin.x.saturating_add(offset),
    y: frame
      .origin
      .y
      .saturating_add(frame.size.height.saturating_sub(tall).wrapping_div(2)),
  };
  let Ok(shape) = words(&text, font, at, width, color);
  shapes.push(shape);
}

fn rectangle(x: i32, y: i32, width: i32, height: i32) -> Rectangle<i32> {
  Rectangle {
    origin: Point { x, y },
    size: Size { width, height },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_color::palette;
  use console_core_fonts::TextStyle;
  use std::error::Error;

  #[test]
  fn engine_dropdown_opens_and_selects_an_available_renderer() -> Result<(), Box<dyn Error>> {
    let Ok(colors) = palette::read(include_str!(
      "../../../files/usr/local/lib/console/palette.sh"
    ));
    let palette = Wearing::out_of(&colors)?;
    let Ok(font) = TextStyle::Footnote.font();
    let size = Size {
      width: 640,
      height: 400,
    };
    let frame = Frame {
      points: size,
      device: size,
    };
    let mut pixels = vec![0; 640 * 400 * 4];
    let status = Status {
      renderer: Renderer::Servo,
      ..Status::default()
    };
    let available = &[Renderer::Servo, Renderer::Chromium];
    let mut settings = Settings::default();
    settings.open();
    settings.paint(&mut pixels, frame, &palette, &font, &status, available)?;
    assert_eq!(
      settings.key(Keysym::Return, 0, &status),
      Action::SettingsChanged,
    );
    settings.paint(&mut pixels, frame, &palette, &font, &status, available)?;
    assert!(
      !settings
        .controls
        .contains(&Control::Engine(Renderer::WebKit))
    );
    assert_eq!(settings.key(Keysym::Down, 0, &status), Action::Handled);
    assert_eq!(
      settings.key(Keysym::Return, 0, &status),
      Action::Switch(Renderer::Chromium),
    );
    settings.paint(&mut pixels, frame, &palette, &font, &status, available)?;
    assert!(
      !settings
        .controls
        .contains(&Control::Engine(Renderer::Chromium))
    );
    assert!(settings.visible());
    Ok(())
  }

  #[test]
  fn tab_glyphs_center_inside_their_buttons() -> Result<(), Box<dyn Error>> {
    let mut shapes = Vec::new();
    let Ok(font) = TextStyle::Footnote.font();
    let Ok(palette) = palette::read(include_str!(
      "../../../files/usr/local/lib/console/palette.sh"
    ));
    let colors = Wearing::out_of(&palette)?;
    let frame = rectangle(12, 50, 28, 29);
    label_at(
      &mut shapes,
      frame,
      "+",
      &font,
      colors.text,
      Alignment::Center,
    );
    let label = match shapes.as_slice() {
      [Shape::Text(label)] => label,
      _ => return Err(std::io::Error::other("expected the plus glyph").into()),
    };
    let Ok(size) = measured("+", &font);
    let Ok(width) = fitted::<_, i32>(size.width);
    assert_eq!(label.at.x, 12 + (28 - width) / 2);
    Ok(())
  }
}
