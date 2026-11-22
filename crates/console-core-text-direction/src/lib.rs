//! The Unicode Bidirectional Algorithm, Unicode Standard Annex #9, over Unicode 16.0.
//!
//! `BidiInfo::new` splits a text into paragraphs and gives every byte of it an embedding level,
//! the number whose parity says which way that character runs; `visual_runs` takes one line of a
//! paragraph and answers its levels made ready for display and the runs of one level in the order
//! they are shown; `reorder_visual` answers that order for levels a caller has worked out itself.
//! Levels and classes are kept per byte, so a character of several bytes has its level several
//! times over, and every position is a byte offset into the text held in a `u32`. A line that is
//! not a run of whole characters inside the text is refused rather than read.
//!
//! The rules are in the order the annex gives them, a module to each part of it: `paragraphs`
//! for P1 to P3, `explicit` for X1 to X8, `sequences` for X10, `weak`, `neutral` and `implicit`
//! for W, N and I, `reordering` for L1 and L2, and `classes` with `tables` for the two character
//! properties they read. A paragraph at level zero with nothing right to left and no formatting
//! character in it stops after the first of them, since every later rule would leave it alone.
//! The formatting characters are given levels rather than removed, as the annex's section on
//! retaining them describes, so the levels line up with the bytes of the text.
//!
//! The crate began as unicode-bidi 0.3.18, by the Servo Project Developers. What it left behind
//! nothing here asked for: the UTF-16 interface, the single-paragraph and deprecated ones, the
//! choice of a data source other than the tables, the base direction of a text on its own, and the
//! convenience that reorders a line's text. It passes every case of the annex's BidiTest.txt and
//! BidiCharacterTest.txt for Unicode 16.0, as upstream did; `tests/the_conformance.rs` carries a
//! sample of both. A change of Unicode version replaces `tables.rs` from the Character Database
//! and is a change of its own.
//!
//! <https://www.unicode.org/reports/tr9/>

#![no_std]

extern crate alloc;

mod classes;
mod explicit;
mod implicit;
mod level;
mod neutral;
mod paragraphs;
mod reordering;
mod sequences;
mod tables;
mod weak;

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, indices};

pub use crate::classes::{BidiClass, bidi_class};
pub use crate::level::{Direction, Level, LevelFault};

pub type LevelRun = Range<u32>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphInfo {
  pub range: Range<u32>,
  pub level: Level,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BidiInfo<'text> {
  text: &'text str,
  original_classes: Vec<BidiClass>,
  levels: Vec<Level>,
  paragraphs: Vec<ParagraphInfo>,
}

#[derive(Debug)]
pub struct Paragraph<'info, 'text> {
  info: &'info BidiInfo<'text>,
  para: &'info ParagraphInfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineFault {
  NotWholeCharacters { line: Range<u32>, length: u32 },
}

impl fmt::Display for LineFault {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotWholeCharacters { line, length } => write!(
        to,
        "the line {}..{} is not a run of whole characters in a text of {length} bytes",
        line.start, line.end
      ),
    }
  }
}

impl core::error::Error for LineFault {}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Place {
  pub(crate) position: u32,
  pub(crate) run: u32,
}

impl<'text> BidiInfo<'text> {
  pub fn new(text: &'text str, default_level: Option<Level>) -> Result<BidiInfo<'text>, Never> {
    let Ok(found) = paragraphs::found(text, default_level);
    let mut levels = Vec::with_capacity(text.len());

    for (paragraph, shape) in found.paragraphs.iter().zip(&found.shapes) {
      let Ok(resolved) = resolved(text, &found.classes, paragraph, *shape);

      levels.extend(resolved);
    }

    Ok(BidiInfo {
      text,
      original_classes: found.classes,
      levels,
      paragraphs: found.paragraphs,
    })
  }

  pub fn text(&self) -> Result<&'text str, Never> {
    Ok(self.text)
  }

  pub fn original_classes(&self) -> Result<&[BidiClass], Never> {
    Ok(&self.original_classes)
  }

  pub fn levels(&self) -> Result<&[Level], Never> {
    Ok(&self.levels)
  }

  pub fn paragraphs(&self) -> Result<&[ParagraphInfo], Never> {
    Ok(&self.paragraphs)
  }

  pub fn into_paragraphs(self) -> Result<Vec<ParagraphInfo>, Never> {
    Ok(self.paragraphs)
  }

  pub fn visual_runs(
    &self,
    paragraph: &ParagraphInfo,
    line: Range<u32>,
  ) -> Result<(Vec<Level>, Vec<LevelRun>), LineFault> {
    let Ok(length) = fitted::<_, u32>(self.text.len());
    let within = match indices(line.clone()) {
      Ok(span) => self.text.get(span),
    };

    match within {
      Some(text) => {
        let mut levels = self.levels.clone();
        let ready = reordering::Line {
          text,
          range: line.clone(),
          paragraph_level: paragraph.level,
        };
        let Ok(()) = reordering::line_levels(&self.original_classes, &mut levels, &ready);
        let Ok(runs) = reordering::runs(&levels, line);

        Ok((levels, runs))
      }
      None => Err(LineFault::NotWholeCharacters { line, length }),
    }
  }

  pub fn reorder_visual(levels: &[Level]) -> Result<Vec<u32>, Never> {
    reordering::visual_order(levels)
  }
}

impl<'info, 'text> Paragraph<'info, 'text> {
  pub fn new(
    info: &'info BidiInfo<'text>,
    para: &'info ParagraphInfo,
  ) -> Result<Paragraph<'info, 'text>, Never> {
    Ok(Paragraph { info, para })
  }

  pub fn details(&self) -> Result<(&'info BidiInfo<'text>, &'info ParagraphInfo), Never> {
    Ok((self.info, self.para))
  }
}

fn resolved(
  text: &str,
  classes: &[BidiClass],
  paragraph: &ParagraphInfo,
  shape: paragraphs::Shape,
) -> Result<Vec<Level>, Never> {
  let Ok(span) = indices(paragraph.range.clone());
  let classes_2 = match classes.get(span.clone()) {
    Some(within) => within,
    None => return Ok(Vec::new()),
  };
  let mut levels = alloc::vec![paragraph.level; classes_2.len()];

  match (paragraph.level, shape.content, text.get(span)) {
    (Level::LEFT_TO_RIGHT, paragraphs::Content::LeftToRightOnly, _) | (_, _, None) => Ok(levels),
    (_, _, Some(text_2)) => {
      let mut processing = classes_2.to_vec();
      let Ok(runs) = explicit::resolved(
        text_2,
        paragraph.level,
        classes_2,
        explicit::Explicit {
          levels: &mut levels,
          processing: &mut processing,
        },
      );
      let held = sequences::Paragraph {
        level: paragraph.level,
        classes: classes_2,
        levels: &levels,
      };
      let Ok(found) = sequences::found(&held, runs, shape.isolates);
      let around = neutral::Around {
        levels: &levels,
        original: classes_2,
      };

      for sequence in &found {
        let Ok(()) = weak::resolved(text_2, sequence, &mut processing);
        let Ok(()) = neutral::resolved(text_2, sequence, &around, &mut processing);
      }

      let Ok(()) = implicit::resolved(&processing, &mut levels);
      let Ok(()) = implicit::removed_follow(paragraph.level, classes_2, &mut levels);

      Ok(levels)
    }
  }
}

pub(crate) fn at<T: Copy>(held: &[T], position: u32) -> Result<Option<T>, Never> {
  let Ok(start) = index(position);

  Ok(held.get(start).copied())
}

pub(crate) fn put<T>(held: &mut [T], position: u32, value: T) -> Result<(), Never> {
  let Ok(start) = index(position);

  match held.get_mut(start) {
    Some(slot) => *slot = value,
    None => {}
  }

  Ok(())
}

pub(crate) fn filled<T: Copy>(held: &mut [T], span: Range<u32>, value: T) -> Result<(), Never> {
  match indices(span) {
    Ok(span_2) => match held.get_mut(span_2) {
      Some(part) => part.fill(value),
      None => {}
    },
  }

  Ok(())
}

pub(crate) fn forwards_from(
  runs: &[LevelRun],
  place: Place,
) -> Result<impl Iterator<Item = u32> + '_, Never> {
  let Ok(run) = index(place.run);
  let current = match runs.get(run) {
    Some(held) => place.position..held.end,
    None => 0..0,
  };
  let later = match runs.get(run.saturating_add(1)..) {
    Some(later) => later,
    None => &[],
  };

  Ok(current.chain(later.iter().cloned().flatten()))
}

pub(crate) fn backwards_from(
  runs: &[LevelRun],
  place: Place,
) -> Result<impl Iterator<Item = u32> + '_, Never> {
  let Ok(run) = index(place.run);
  let current = match runs.get(run) {
    Some(held) => held.start..place.position,
    None => 0..0,
  };
  let earlier = match runs.get(..run) {
    Some(earlier) => earlier,
    None => &[],
  };

  Ok(current.rev().chain(earlier.iter().rev().cloned().flatten()))
}
