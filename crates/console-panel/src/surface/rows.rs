//! Measure wrapped labels and space between groups once for drawing, focus,
//! scrolling and hit testing.

use std::ops::Range;

use crate::fitting;
use crate::page::{Row, Wears};
use crate::row_window::{RowViewport, visible_rows};
use crate::strip;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::{
  BETWEEN_ROWS, Band, Card, FINGER, INSIDE_A_ROW, MeasuredMarks, Point, RowText, Run,
  SELECTED_MARK, Shape, Text, TextStyle, Weight, beside_the_words, font, inside_left,
  inside_right, painting, width,
};

pub(super) enum LayoutCache {
  Unmeasured,
  Measured { width: i32, layout: RowLayout },
}

impl LayoutCache {
  pub(super) fn layout(&self) -> Result<&RowLayout, Never> {
    Ok(match self {
      Self::Unmeasured => &RowLayout::Uniform,
      Self::Measured { layout, .. } => layout,
    })
  }

  pub(super) fn measure(
    &mut self,
    rows: &[Row],
    card: &Card,
    marks: MeasuredMarks,
  ) -> Result<(), Never> {
    match self {
      Self::Measured { width, .. } => match (*width == card.width).then_some(()) {
        Some(()) => return Ok(()),
        None => {}
      },
      Self::Unmeasured => {}
    }

    let Ok(layout) = RowLayout::measure(rows, card, marks);
    *self = Self::Measured {
      width: card.width,
      layout,
    };

    Ok(())
  }
}

pub(super) enum RowLayout {
  Uniform,
  Wrapped(Vec<WrappedRow>),
}

pub(super) struct WrappedRow {
  band: Band,
  stacked: Option<StackedText>,
}

impl RowLayout {
  pub(super) fn measure(rows: &[Row], card: &Card, marks: MeasuredMarks) -> Result<Self, Never> {
    match rows
      .iter()
      .any(|row| row.stacked || row.space_above)
      .then_some(())
    {
      Some(()) => {}
      None => return Ok(Self::Uniform),
    }

    let Ok(left) = inside_left(card);
    let Ok(right) = inside_right(card);
    let mut top = 0i32;
    let mut measured = Vec::with_capacity(rows.len());

    for row in rows {
      match row.space_above.then_some(()) {
        Some(()) => top = top.saturating_add(strip::GAP.saturating_mul(2)),
        None => {}
      }

      let stacked = match row.stacked.then_some(()) {
        Some(()) => {
          let Ok(wide) = label_width(row, right.saturating_sub(left), marks);
          let Ok(text) = StackedText::measure(row, wide);
          Some(text)
        }
        None => None,
      };
      let height = match stacked {
        Some(text) => text
          .height
          .saturating_add(strip::GAP.saturating_mul(2))
          .saturating_add(BETWEEN_ROWS)
          .max(fitting::ROW),
        None => fitting::ROW,
      };
      measured.push(WrappedRow {
        band: Band { top, height },
        stacked,
      });
      top = top.saturating_add(height);
    }

    Ok(Self::Wrapped(measured))
  }

  pub(super) fn row(&self, at: u64) -> Result<(Band, Option<StackedText>), Never> {
    match self {
      Self::Uniform => {}
      Self::Wrapped(rows) => {
        let Ok(slot) = index(at);

        match rows.get(slot) {
          Some(row) => return Ok((row.band, row.stacked)),
          None => {}
        }
      }
    }

    let Ok(position) = fitted::<u64, i32>(at);
    Ok((
      Band {
        top: position.saturating_mul(fitting::ROW),
        height: fitting::ROW,
      },
      None,
    ))
  }

  pub(super) fn height(&self, count: u64) -> Result<i32, Never> {
    Ok(match self {
      Self::Uniform => {
        let Ok(many) = fitted::<u64, i32>(count);
        many.saturating_mul(fitting::ROW)
      }
      Self::Wrapped(rows) => match rows.last() {
        Some(row) => row.band.top.saturating_add(row.band.height),
        None => 0,
      },
    })
  }

  pub(super) fn visible(&self, viewport: RowViewport) -> Result<Range<u64>, Never> {
    let rows = match self {
      Self::Uniform => return visible_rows(viewport),
      Self::Wrapped(rows) => rows,
    };

    match (viewport.height <= 0).then_some(()) {
      Some(()) => return Ok(0..0),
      None => {}
    }

    let bottom = |row: &WrappedRow| {
      row
        .band
        .top
        .saturating_add(row.band.height)
        .saturating_add(viewport.stage)
    };
    let end = viewport.scroll.saturating_add(viewport.height);
    let Ok(first) = fitted::<_, u64>(rows.partition_point(|row| bottom(row) <= viewport.scroll));
    let Ok(last) = fitted::<_, u64>(rows.partition_point(|row| bottom(row) < end));
    let Ok(count) = fitted::<_, u64>(rows.len());

    Ok(first..last.saturating_add(1).min(count))
  }
}

fn label_width(row: &Row, inside: i32, marks: MeasuredMarks) -> Result<u32, Never> {
  let Ok(picture) = beside_the_words(&row.picture);
  let Ok(wears) = crate::page::wears(row);
  let end = match wears {
    Wears::WhatElse => FINGER.saturating_add(strip::GAP.saturating_mul(3)),
    Wears::None => INSIDE_A_ROW,
  };
  let arrow = match row.opens.then_some(()) {
    Some(()) => {
      let Ok(into) = fitted::<u32, i32>(marks.into.width);
      into.saturating_add(strip::GAP.saturating_mul(3))
    }
    None => 0,
  };
  let selected = match row.key.as_ref() {
    Some(_) => SELECTED_MARK.saturating_add(strip::GAP.saturating_mul(3)),
    None => 0,
  };

  width(
    inside
      .saturating_sub(INSIDE_A_ROW)
      .saturating_sub(picture)
      .saturating_sub(end)
      .saturating_sub(arrow)
      .saturating_sub(selected)
      .max(1),
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StackedText {
  width: u32,
  height: i32,
  detail_at: i32,
}

impl StackedText {
  fn measure(row: &Row, width: u32) -> Result<Self, Never> {
    let Ok(body) = font(TextStyle::Body);
    let Ok(title) = painting::measure_text(
      Run {
        said: &row.says,
        weight: Weight::Bold,
        width,
      },
      &body,
    );
    let Ok(title_height) = fitted::<u32, i32>(title.height);
    let detail_at = title_height.saturating_add(strip::GAP);
    let height = match row.aside.is_empty().then_some(()) {
      Some(()) => title_height,
      None => {
        let Ok(callout) = font(TextStyle::Callout);
        let Ok(detail) = painting::measure_text(
          Run {
            said: &row.aside,
            weight: Weight::Plain,
            width,
          },
          &callout,
        );
        let Ok(detail_height) = fitted::<u32, i32>(detail.height);
        detail_at.saturating_add(detail_height)
      }
    };

    Ok(Self {
      width,
      height,
      detail_at,
    })
  }

  pub(super) fn draw(
    self,
    row: &Row,
    text: RowText,
    shapes: &mut Vec<Shape>,
  ) -> Result<(), Never> {
    let top = text.band.top.saturating_add(
      text
        .band
        .height
        .saturating_sub(self.height)
        .saturating_div(2),
    );
    let Ok(body) = font(TextStyle::Body);
    shapes.push(Shape::Text(Text {
      at: Point {
        x: text.words_x,
        y: top,
      },
      width: self.width,
      said: row.says.clone(),
      weight: Weight::Bold,
      font: body,
      ink: text.ink,
    }));

    match row.aside.is_empty().then_some(()) {
      Some(()) => {}
      None => {
        let Ok(callout) = font(TextStyle::Callout);
        shapes.push(Shape::Text(Text {
          at: Point {
            x: text.words_x,
            y: top.saturating_add(self.detail_at),
          },
          width: self.width,
          said: row.aside.clone(),
          weight: Weight::Plain,
          font: callout,
          ink: text.quiet,
        }));
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::page::Aside;

  #[test]
  fn space_between_rows_is_outside_the_following_row() {
    let Ok(recording) = Row::text("Recording", Aside(""));
    let Ok(search) = Row::text("Search", Aside(""));
    let Ok(separated) = search.with_space_above();
    let card = Card {
      x: 0,
      y: 0,
      width: 640,
      height: 360,
    };
    let Ok(marks) = super::super::measure_marks();
    let Ok(layout) = RowLayout::measure(&[recording, separated], &card, marks);
    let Ok((first, _)) = layout.row(0);
    let Ok((second, _)) = layout.row(1);

    assert_eq!(first.top, 0);
    assert_eq!(second.height, first.height);
    assert_eq!(
      second.top.saturating_sub(first.height),
      strip::GAP.saturating_mul(2)
    );
  }
}
