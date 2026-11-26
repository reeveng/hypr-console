//! Painting and pointer placement for a multiline editor with caller-supplied rows.

use std::ops::Range;

use console_core_color::Oklch;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Font, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Run, measure_text};
use console_input_text::{Row, TextArea, between};

pub struct Line {
  pub row: Row,
  pub at: Point<i32>,
  pub size: Size<u32>,
  pub font: Font,
  pub weight: Weight,
}

#[derive(Clone, Copy)]
pub struct Colors {
  pub text: Oklch,
  pub background: Oklch,
  pub caret: Oklch,
  pub placeholder: Oklch,
}

pub struct View<'a> {
  pub area: &'a TextArea,
  pub lines: &'a [Line],
  pub hint: &'a str,
}

impl View<'_> {
  pub fn caret_at(&self, at: Point<i32>) -> Result<u32, Never> {
    let nearest = self.lines.iter().min_by_key(|line| {
      let Ok(half) = fitted::<u32, i32>(line.size.height.saturating_div(2));
      line.at.y.saturating_add(half).abs_diff(at.y)
    });
    let line = match nearest {
      Some(line) => line,
      None => return self.area.caret(),
    };
    let Ok(text) = self.area.text();
    let Ok(said) = between(text, line.row.start..line.row.end);
    let Ok(local) = caret_position(
      said,
      &line.font,
      line.weight,
      at.x.saturating_sub(line.at.x).max(0).unsigned_abs(),
    );

    Ok(line.row.start.saturating_add(local))
  }

  pub fn paint(&self, shapes: &mut Vec<Shape>, colors: Colors) -> Result<(), Never> {
    let Ok(text) = self.area.text();
    let Ok(caret) = self.area.caret();
    let Ok(selection) = self.area.selection();
    let caret_row = self
      .lines
      .iter()
      .rev()
      .find(|line| line.row.start <= caret && caret <= line.row.end);
    let Ok(fill) = colors
      .background
      .at(colors.background.lightness * 0.8 + colors.text.lightness * 0.2);

    for line in self.lines {
      let Ok(line_text) = between(text, line.row.start..line.row.end);
      let (said, ink) = match (text.is_empty(), line.row.start) {
        (true, 0) => (self.hint, colors.placeholder),
        _ => (line_text, colors.text),
      };
      let Ok(()) = line.highlight(shapes, text, selection.as_ref(), fill);
      shapes.push(Shape::Text(Text {
        at: line.at,
        width: line.size.width,
        said: said.to_owned(),
        weight: line.weight,
        font: line.font.clone(),
        ink,
      }));

      match caret_row.filter(|active| std::ptr::eq(*active, line)) {
        Some(_) => {
          let Ok(offset) = line.offset(text, caret);
          let Ok(()) = line.marker(shapes, offset..offset.saturating_add(2), colors.caret);
        }
        None => {}
      }
    }

    Ok(())
  }
}

impl Line {
  fn offset(&self, text: &str, position: u32) -> Result<u32, Never> {
    let Ok(before) = between(text, self.row.start..position);
    advance(before, &self.font, self.weight)
  }

  fn highlight(
    &self,
    shapes: &mut Vec<Shape>,
    text: &str,
    selection: Option<&Range<u32>>,
    fill: Oklch,
  ) -> Result<(), Never> {
    let range = match selection {
      Some(range) => range,
      None => return Ok(()),
    };
    let start = range.start.max(self.row.start);
    let end = range.end.min(self.row.end);

    match (start < end).then_some(()) {
      Some(()) => {
        let Ok(from) = self.offset(text, start);
        let Ok(to) = self.offset(text, end);
        self.marker(shapes, from..to, fill)
      }
      None => Ok(()),
    }
  }

  fn marker(
    &self,
    shapes: &mut Vec<Shape>,
    pixels: Range<u32>,
    fill: Oklch,
  ) -> Result<(), Never> {
    let Ok(offset) = fitted::<u32, i32>(pixels.start);
    shapes.push(Shape::Panel(Panel {
      at: Point {
        x: self.at.x.saturating_add(offset),
        ..self.at
      },
      size: Size {
        width: pixels.end.saturating_sub(pixels.start),
        height: self.size.height,
      },
      round: Round(0),
      fill,
      edge: Edge::None,
    }));
    Ok(())
  }
}

pub fn caret_position(said: &str, font: &Font, weight: Weight, x: u32) -> Result<u32, Never> {
  let Ok(letters) = console_input_text::grapheme_count(said);
  let mut previous = 0_u32;

  for caret in 1..=letters {
    let Ok(prefix) = between(said, 0..caret);
    let Ok(right) = advance(prefix, font, weight);
    let middle = previous.saturating_add(right.saturating_sub(previous).div_ceil(2));

    match (x < middle).then_some(()) {
      Some(()) => return Ok(caret.saturating_sub(1)),
      None => previous = right,
    }
  }

  Ok(letters)
}

fn advance(said: &str, font: &Font, weight: Weight) -> Result<u32, Never> {
  let Ok(measured) = measure_text(
    Run {
      said,
      weight,
      width: u32::MAX,
    },
    font,
  );
  Ok(measured.width)
}
