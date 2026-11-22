//! The first pass over the text: where each paragraph ends, its level, and the class of every byte.
//!
//! A paragraph separator closes the paragraph it ends and is kept with it (P1). A paragraph's
//! level is the one the caller asked for, or else the direction of its first strong character
//! outside an isolate (P2), or left to right where there is none (P3). A first strong isolate is
//! resolved here as well: the first strong character between it and its matching PDI makes it a
//! right-to-left isolate or a left-to-right one (X5c), and it is written into the classes as that,
//! so one left unresolved has no strong character before its PDI and is treated as LRI later.
//!
//! Each paragraph is also told whether it can skip the rest of the algorithm: one at level zero
//! with nothing right to left and no formatting character in it has every level it will ever have.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::iter::repeat_n;
use core::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::ParagraphInfo;
use crate::classes::{BidiClass, bidi_class};
use crate::level::{Direction, Level};

const FIRST_STRONG_ISOLATE: char = '\u{2068}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Content {
  LeftToRightOnly,
  Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Isolates {
  Absent,
  Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shape {
  pub(crate) content: Content,
  pub(crate) isolates: Isolates,
}

pub(crate) struct Found {
  pub(crate) classes: Vec<BidiClass>,
  pub(crate) paragraphs: Vec<ParagraphInfo>,
  pub(crate) shapes: Vec<Shape>,
}

struct Scan {
  asked: Option<Level>,
  start: u32,
  level: Option<Level>,
  shape: Shape,
  open_isolates: Vec<u32>,
}

const FRESH: Shape = Shape {
  content: Content::LeftToRightOnly,
  isolates: Isolates::Absent,
};

pub(crate) fn found(text: &str, asked: Option<Level>) -> Result<Found, Never> {
  let mut held = Found {
    classes: Vec::with_capacity(text.len()),
    paragraphs: Vec::new(),
    shapes: Vec::new(),
  };
  let mut scan = Scan {
    asked,
    start: 0,
    level: asked,
    shape: FRESH,
    open_isolates: Vec::new(),
  };

  for (at, character) in text.char_indices() {
    let Ok(position) = fitted::<_, u32>(at);
    let Ok(()) = scanned(&mut held, &mut scan, position, character);
  }

  let Ok(end) = fitted::<_, u32>(text.len());

  match scan.start.cmp(&end) {
    Ordering::Less => {
      let Ok(()) = closed(&mut held, &scan, scan.start..end);
    }
    Ordering::Equal | Ordering::Greater => {}
  }

  Ok(held)
}

fn scanned(
  held: &mut Found,
  scan: &mut Scan,
  position: u32,
  character: char,
) -> Result<(), Never> {
  let Ok(class) = bidi_class(character);
  let Ok(width) = fitted::<_, u32>(character.len_utf8());

  held.classes.extend(repeat_n(class, character.len_utf8()));

  match class {
    BidiClass::B => {
      let end = position.saturating_add(width);
      let Ok(()) = closed(held, scan, scan.start..end);

      scan.start = end;
      scan.level = scan.asked;
      scan.shape = FRESH;
      scan.open_isolates.clear();
    }
    BidiClass::L => {
      let Ok(()) = strong(held, scan, Direction::LeftToRight);
    }
    BidiClass::R | BidiClass::AL => {
      scan.shape.content = Content::Mixed;
      let Ok(()) = strong(held, scan, Direction::RightToLeft);
    }
    BidiClass::AN | BidiClass::LRE | BidiClass::RLE | BidiClass::LRO | BidiClass::RLO => {
      scan.shape.content = Content::Mixed;
    }
    BidiClass::RLI | BidiClass::LRI | BidiClass::FSI => {
      scan.shape = Shape {
        content: Content::Mixed,
        isolates: Isolates::Present,
      };
      scan.open_isolates.push(position);
    }
    BidiClass::PDI => {
      let _closed_isolate = scan.open_isolates.pop();
    }
    BidiClass::BN
    | BidiClass::CS
    | BidiClass::EN
    | BidiClass::ES
    | BidiClass::ET
    | BidiClass::NSM
    | BidiClass::ON
    | BidiClass::PDF
    | BidiClass::S
    | BidiClass::WS => {}
  }

  Ok(())
}

fn strong(held: &mut Found, scan: &mut Scan, direction: Direction) -> Result<(), Never> {
  match scan.open_isolates.last() {
    Some(&isolate) => {
      let Ok(start) = index(isolate);

      match held.classes.get(start) {
        Some(BidiClass::FSI) => {}
        Some(_) | None => return Ok(()),
      }

      let resolved = match direction {
        Direction::LeftToRight => BidiClass::LRI,
        Direction::RightToLeft => BidiClass::RLI,
      };

      match held
        .classes
        .get_mut(start..start.saturating_add(FIRST_STRONG_ISOLATE.len_utf8()))
      {
        Some(bytes) => bytes.fill(resolved),
        None => {}
      }
    }
    None => match scan.level {
      Some(_) => {}
      None => {
        scan.level = Some(match direction {
          Direction::LeftToRight => Level::LEFT_TO_RIGHT,
          Direction::RightToLeft => Level::RIGHT_TO_LEFT,
        });
      }
    },
  }

  Ok(())
}

fn closed(held: &mut Found, scan: &Scan, range: Range<u32>) -> Result<(), Never> {
  let level = match scan.level {
    Some(level) => level,
    None => Level::LEFT_TO_RIGHT,
  };

  held.paragraphs.push(ParagraphInfo { range, level });
  held.shapes.push(scan.shape);

  Ok(())
}
