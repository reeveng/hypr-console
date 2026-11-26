//! The single-line editor shared by panels and application toolbars.

use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_shapes::{Clip, Edge, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Run, measure_text};
use console_draw_surface::{Composition, Part};
pub use console_input_text::{Row, TextArea, TextField, Typed, Way};

use crate::{
  canvas::{Area, AreaLayout},
  strip,
};

pub const HEIGHT: i32 = strip::HEIGHT;
const PADDING: i32 = 16;
const CARET_WIDTH: u32 = 2;

pub enum Content<'a> {
  Value(&'a str),
  Placeholder(&'a str),
}

#[derive(Clone, Copy)]
pub enum Editing {
  Inactive,
  Selected,
  Range {
    caret: u32,
    start: u32,
    end: u32,
  },
  Active {
    caret: Option<u32>,
    preedit: Option<(u32, u32)>,
  },
}

pub mod area;
pub mod clipboard;
pub mod gesture;
pub mod menu;

pub enum Editor<'a> {
  Field(&'a mut TextField),
  Password(&'a mut TextField),
  Area(&'a mut TextArea),
}

impl Editor<'_> {
  pub fn select(self, position: u32, selection: gesture::Selection) -> Result<Typed, Never> {
    use gesture::Selection;

    match (self, selection) {
      (Self::Field(field) | Self::Password(field), Selection::Place) => field.moved_to(position),
      (Self::Area(area), Selection::Place) => area.placed(position),
      (Self::Field(field) | Self::Password(field), Selection::Extend) => {
        field.extended_to(position)
      }
      (Self::Area(area), Selection::Extend) => area.extended_to(position),
      (Self::Field(field), Selection::Word) => field.select_word(position),
      (Self::Area(area), Selection::Word) => area.select_word(position),
      (Self::Password(field), Selection::Word) => field.moved_to(position),
    }
  }

  pub fn clipboard(
    self,
    shortcut: clipboard::Shortcut,
    clipboard: &mut clipboard::Clipboard,
  ) -> Result<(), Never> {
    use clipboard::Shortcut;

    match shortcut {
      Shortcut::Paste => clipboard.paste(),
      Shortcut::Copy | Shortcut::Cut => self.copy(shortcut, clipboard),
      Shortcut::Repeat | Shortcut::Other => Ok(()),
    }
  }

  fn copy(
    self,
    shortcut: clipboard::Shortcut,
    clipboard: &mut clipboard::Clipboard,
  ) -> Result<(), Never> {
    let Ok(selected) = match &self {
      Self::Field(field) => field.selected_text(),
      Self::Area(area) => area.selected_text(),
      Self::Password(_) => Ok(None),
    };

    match (selected, shortcut) {
      (Some(text), clipboard::Shortcut::Copy) => clipboard.copy(text),
      (Some(text), clipboard::Shortcut::Cut) => clipboard.cut(text),
      _ => Ok(()),
    }
  }

  pub fn change(self, change: clipboard::Change) -> Result<Typed, Never> {
    match (self, change) {
      (Self::Field(field) | Self::Password(field), clipboard::Change::Paste(text)) => {
        field.paste(&text)
      }
      (Self::Area(area), clipboard::Change::Paste(text)) => area.paste(&text),
      (Self::Field(field), clipboard::Change::Cut) => field.delete_selection(),
      (Self::Area(area), clipboard::Change::Cut) => area.delete_selection(),
      (Self::Password(_), clipboard::Change::Cut) => Ok(Typed::None),
    }
  }
}

pub struct TextParts<'a> {
  pub before: &'a str,
  pub after: &'a str,
}

pub struct Draft {
  pub text: String,
  pub editing: Editing,
}

impl Draft {
  #[must_use]
  pub fn new(parts: TextParts<'_>, preedit: Option<&Composition>) -> Result<Self, Never> {
    let TextParts { before, after } = parts;
    let Ok(start) = measure(before);
    let (text, editing) = match preedit {
      Some(composition) => {
        let inserted = format!("{before}{}", composition.text);
        let Ok(end) = measure(&inserted);
        let caret =
          match (composition.cursor_begin >= 0 || composition.cursor_end >= 0).then_some(()) {
            Some(()) => {
              let Ok(offset) = index(composition.cursor_end.max(0));
              let prefix = match composition.text.get(..offset) {
                Some(prefix) => prefix,
                None => composition.text.as_str(),
              };
              let Ok(through) = measure(&format!("{before}{prefix}"));
              Some(through.width)
            }
            None => None,
          };
        (
          format!("{inserted}{after}"),
          Editing::Active {
            caret,
            preedit: Some((start.width, end.width)),
          },
        )
      }
      None => (
        format!("{before}{after}"),
        Editing::Active {
          caret: Some(start.width),
          preedit: None,
        },
      ),
    };
    Ok(Self { text, editing })
  }
}

pub struct Field<'a> {
  pub area: Area,
  pub trailing: i32,
  pub content: Content<'a>,
  pub editing: Editing,
}

#[must_use]
pub fn measure(said: &str) -> Result<Size<u32>, Never> {
  let Ok(font) = TextStyle::Body.font();
  measure_text(
    Run {
      said,
      weight: Weight::Plain,
      width: u32::MAX,
    },
    &font,
  )
}

struct Line {
  area: Area,
  origin: Point<i32>,
  height: u32,
}

impl Line {
  fn at(&self, width: u32) -> Result<Point<i32>, Never> {
    let Ok(offset) = fitted::<u32, i32>(width);
    Ok(Point {
      x: self.origin.x.saturating_add(offset),
      y: self.origin.y,
    })
  }
}

impl Field<'_> {
  pub fn caret_at(&self, said: &str, point: Point<i32>) -> Result<u32, Never> {
    let Ok(line) = self.layout();
    let Ok(font) = TextStyle::Body.font();
    let x = point.x.saturating_sub(line.origin.x).max(0).unsigned_abs();
    area::caret_position(said, &font, Weight::Plain, x)
  }

  fn layout(&self) -> Result<Line, Never> {
    let Ok(metrics) = measure("M");
    let Ok(height) = fitted::<u32, i32>(metrics.height);
    let line = Area {
      origin: Point {
        x: self.area.origin.x.saturating_add(PADDING),
        y: self.area.origin.y,
      },
      size: Size {
        width: self
          .area
          .size
          .width
          .saturating_sub(PADDING.saturating_mul(2))
          .saturating_sub(self.trailing)
          .max(0),
        height: self.area.size.height,
      },
    };
    let caret = match self.editing {
      Editing::Active {
        caret: Some(width), ..
      } => width,
      Editing::Range { caret, .. } => caret,
      Editing::Active { caret: None, .. } | Editing::Inactive | Editing::Selected => 0,
    };
    let overflow =
      caret.saturating_sub(line.size.width.unsigned_abs().saturating_sub(CARET_WIDTH));
    let Ok(shift) = fitted::<u32, i32>(overflow);
    Ok(Line {
      area: line,
      origin: Point {
        x: line.origin.x.saturating_sub(shift),
        y: line
          .origin
          .y
          .saturating_add(line.size.height.saturating_sub(height).saturating_div(2)),
      },
      height: metrics.height,
    })
  }

  fn cursor(&self, line: &Line) -> Result<Option<Part>, Never> {
    Ok(match self.editing {
      Editing::Active {
        caret: Some(width), ..
      } => {
        let Ok(at) = line.at(width);
        Some(Part {
          at,
          size: Size {
            width: CARET_WIDTH,
            height: line.height,
          },
        })
      }
      Editing::Active { caret: None, .. }
      | Editing::Inactive
      | Editing::Selected
      | Editing::Range { .. } => None,
    })
  }

  #[must_use]
  pub fn caret(&self) -> Result<Option<Part>, Never> {
    let Ok(line) = self.layout();
    self.cursor(&line)
  }

  pub fn paint(&self, shapes: &mut Vec<Shape>, colors: &Wearing) -> Result<Option<Part>, Never> {
    let Ok(size) = self.area.pixel_size();
    let edge = match self.editing {
      Editing::Inactive => Edge::Of {
        wide: 1,
        color: colors.panel,
      },
      Editing::Selected | Editing::Active { .. } | Editing::Range { .. } => Edge::Of {
        wide: 2,
        color: colors.pink,
      },
    };
    shapes.push(Shape::Panel(Panel {
      at: self.area.origin,
      size,
      round: Round(strip::ROUND),
      fill: colors.night,
      edge,
    }));
    let Ok(line) = self.layout();
    let Ok(clip_size) = line.area.pixel_size();
    shapes.push(Shape::Clip(Clip::To {
      at: line.area.origin,
      size: clip_size,
    }));
    let (said, ink) = match self.content {
      Content::Value(written) => (written, colors.text),
      Content::Placeholder(hint) => (hint, colors.soft),
    };

    let highlight = match self.editing {
      Editing::Selected => {
        let Ok(width) = measure(said);
        Some((0, width.width))
      }
      Editing::Range { start, end, .. } => Some((start, end)),
      Editing::Active { .. } | Editing::Inactive => None,
    };

    match highlight {
      Some((start, end)) => {
        let Ok(at) = line.at(start);
        shapes.push(Shape::Panel(Panel {
          at,
          size: Size {
            width: end.saturating_sub(start),
            height: line.height,
          },
          round: Round(2),
          fill: colors.fill,
          edge: Edge::None,
        }));
      }
      None => {}
    }

    let Ok(font) = TextStyle::Body.font();
    shapes.push(Shape::Text(Text {
      at: line.origin,
      width: u32::MAX,
      said: said.to_owned(),
      weight: Weight::Plain,
      font,
      ink,
    }));

    match self.editing {
      Editing::Active {
        preedit: Some((start, end)),
        ..
      } => {
        let Ok(from) = line.at(start);
        let Ok(height) = fitted::<u32, i32>(line.height);
        let at = Point {
          x: from.x,
          y: from.y.saturating_add(height),
        };
        shapes.push(Shape::Panel(Panel {
          at,
          size: Size {
            width: end.saturating_sub(start),
            height: 2,
          },
          round: Round(0),
          fill: colors.pink,
          edge: Edge::None,
        }));
      }
      Editing::Active { preedit: None, .. }
      | Editing::Selected
      | Editing::Inactive
      | Editing::Range { .. } => {}
    }

    let Ok(cursor) = self.cursor(&line);

    match cursor.as_ref() {
      Some(part) => shapes.push(Shape::Panel(Panel {
        at: part.at,
        size: part.size,
        round: Round(0),
        fill: colors.pink,
        edge: Edge::None,
      })),
      None => {}
    }

    shapes.push(Shape::Clip(Clip::Lifted));
    Ok(cursor)
  }
}
