//! The explicit levels and directions (X1 to X8), and the level runs they leave (BD7).
//!
//! The directional status stack starts with the paragraph's own entry, which nothing pops, so it
//! is held apart from the entries above it. An embedding, an override or an isolate pushes an
//! entry while the level it asks for is no deeper than 125 and nothing has overflowed; past that
//! it is counted as an overflow instead, and the matching PDF or PDI uncounts it. Where an isolate
//! entry sits in the stack is kept as it is pushed, so a PDI pops back through its isolate by
//! cutting the stack there rather than by looking for it.
//!
//! The formatting characters keep a level here, as the annex's section on retaining them asks:
//! each takes the level of the entry it was read under, and an embedding or override that pushed
//! takes its own new one. Every byte of a character gets the character's level and class. A new
//! level run starts at each character rule X9 does not remove whose level differs from the run's.
//!
//! <https://www.unicode.org/reports/tr9/#Explicit_Levels_and_Directions>

use alloc::vec::Vec;
use core::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::classes::{BidiClass, Retention, retention};
use crate::level::{Direction, Level};
use crate::{LevelRun, at, filled};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
  Neutral,
  RightToLeft,
  LeftToRight,
  Isolate,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
  level: Level,
  status: Status,
}

struct Stack {
  bottom: Entry,
  above: Vec<Entry>,
  isolates: Vec<u32>,
  overflow_isolates: u32,
  overflow_embeddings: u32,
}

struct Set {
  level: Option<Level>,
  class: Option<BidiClass>,
}

struct Run {
  start: u32,
  level: Level,
}

pub(crate) struct Explicit<'held> {
  pub(crate) levels: &'held mut [Level],
  pub(crate) processing: &'held mut [BidiClass],
}

pub(crate) fn resolved(
  text: &str,
  paragraph_level: Level,
  classes: &[BidiClass],
  into: Explicit<'_>,
) -> Result<Vec<LevelRun>, Never> {
  let mut stack = Stack {
    bottom: Entry {
      level: paragraph_level,
      status: Status::Neutral,
    },
    above: Vec::new(),
    isolates: Vec::new(),
    overflow_isolates: 0,
    overflow_embeddings: 0,
  };
  let mut runs = Vec::new();
  let mut run = Run {
    start: 0,
    level: Level::LEFT_TO_RIGHT,
  };

  for (start, character) in text.char_indices() {
    let Ok(position) = fitted::<_, u32>(start);
    let Ok(width) = fitted::<_, u32>(character.len_utf8());
    let Ok(found) = at(classes, position);

    match found {
      Some(class) => {
        let Ok(set) = stepped(&mut stack, class);
        let Ok(level) = written(
          into.levels,
          position..position.saturating_add(width),
          set.level,
        );
        let Ok(_class) = written(
          into.processing,
          position..position.saturating_add(width),
          set.class,
        );
        let Ok(()) = ran(&mut runs, &mut run, position, class, level);
      }
      None => {}
    }
  }

  let Ok(end) = fitted::<_, u32>(into.levels.len());

  match end.cmp(&run.start) {
    Ordering::Greater => runs.push(run.start..end),
    Ordering::Less | Ordering::Equal => {}
  }

  Ok(runs)
}

fn stepped(stack: &mut Stack, class: BidiClass) -> Result<Set, Never> {
  let Ok(top) = stack.top();

  match class {
    BidiClass::RLE => initiated(stack, top, Direction::RightToLeft, Status::Neutral),
    BidiClass::LRE => initiated(stack, top, Direction::LeftToRight, Status::Neutral),
    BidiClass::RLO => initiated(stack, top, Direction::RightToLeft, Status::RightToLeft),
    BidiClass::LRO => initiated(stack, top, Direction::LeftToRight, Status::LeftToRight),
    BidiClass::RLI => initiated(stack, top, Direction::RightToLeft, Status::Isolate),
    BidiClass::LRI | BidiClass::FSI => {
      initiated(stack, top, Direction::LeftToRight, Status::Isolate)
    }
    BidiClass::PDI => isolate_closed(stack),
    BidiClass::PDF => embedding_closed(stack, top),
    BidiClass::B => Ok(Set {
      level: None,
      class: None,
    }),
    BidiClass::BN => Ok(Set {
      level: Some(top.level),
      class: None,
    }),
    BidiClass::L
    | BidiClass::R
    | BidiClass::AL
    | BidiClass::EN
    | BidiClass::ES
    | BidiClass::ET
    | BidiClass::AN
    | BidiClass::CS
    | BidiClass::NSM
    | BidiClass::S
    | BidiClass::WS
    | BidiClass::ON => {
      let Ok(overridden) = top.status.overriding();

      Ok(Set {
        level: Some(top.level),
        class: overridden,
      })
    }
  }
}

fn initiated(
  stack: &mut Stack,
  top: Entry,
  direction: Direction,
  status: Status,
) -> Result<Set, Never> {
  let Ok(next) = top.level.next_explicit(direction);
  let Ok(overridden) = top.status.overriding();
  let processed = match status {
    Status::Isolate => overridden,
    Status::Neutral | Status::RightToLeft | Status::LeftToRight => Some(BidiClass::BN),
  };
  let level = match (next, stack.overflow_isolates, stack.overflow_embeddings) {
    (Some(deeper), 0, 0) => {
      let level = match status {
        Status::Isolate => {
          let Ok(depth) = fitted::<_, u32>(stack.above.len());

          stack.isolates.push(depth);
          top.level
        }
        Status::Neutral | Status::RightToLeft | Status::LeftToRight => deeper,
      };

      stack.above.push(Entry {
        level: deeper,
        status,
      });

      level
    }
    (_, _, _) => {
      match (status, stack.overflow_isolates) {
        (Status::Isolate, _) => {
          stack.overflow_isolates = stack.overflow_isolates.saturating_add(1)
        }
        (Status::Neutral | Status::RightToLeft | Status::LeftToRight, 0) => {
          stack.overflow_embeddings = stack.overflow_embeddings.saturating_add(1);
        }
        (Status::Neutral | Status::RightToLeft | Status::LeftToRight, _) => {}
      }

      top.level
    }
  };

  Ok(Set {
    level: Some(level),
    class: processed,
  })
}

fn isolate_closed(stack: &mut Stack) -> Result<Set, Never> {
  match stack.overflow_isolates.checked_sub(1) {
    Some(fewer) => stack.overflow_isolates = fewer,
    None => match stack.isolates.pop() {
      Some(depth) => {
        let Ok(kept) = index(depth);

        stack.overflow_embeddings = 0;
        stack.above.truncate(kept);
      }
      None => {}
    },
  }

  let Ok(top) = stack.top();
  let Ok(overridden) = top.status.overriding();

  Ok(Set {
    level: Some(top.level),
    class: overridden,
  })
}

fn embedding_closed(stack: &mut Stack, top: Entry) -> Result<Set, Never> {
  match (
    stack.overflow_isolates,
    stack.overflow_embeddings.checked_sub(1),
  ) {
    (0, Some(fewer)) => stack.overflow_embeddings = fewer,
    (0, None) => match top.status {
      Status::Isolate => {}
      Status::Neutral | Status::RightToLeft | Status::LeftToRight => {
        let _closed_embedding = stack.above.pop();
      }
    },
    (_, _) => {}
  }

  let Ok(now) = stack.top();

  Ok(Set {
    level: Some(now.level),
    class: Some(BidiClass::BN),
  })
}

fn written<T: Copy>(
  held: &mut [T],
  span: core::ops::Range<u32>,
  asked: Option<T>,
) -> Result<Option<T>, Never> {
  let Ok(current) = at(held, span.start);
  let value = asked.or(current);

  match value {
    Some(value_2) => {
      let Ok(()) = filled(held, span, value_2);
    }
    None => {}
  }

  Ok(value)
}

fn ran(
  runs: &mut Vec<LevelRun>,
  run: &mut Run,
  position: u32,
  class: BidiClass,
  level: Option<Level>,
) -> Result<(), Never> {
  let Ok(kept) = retention(class);

  match (position, level) {
    (0, Some(first)) => run.level = first,
    (_, Some(level_2)) => match (kept, level_2.cmp(&run.level)) {
      (Retention::Retained, Ordering::Less | Ordering::Greater) => {
        runs.push(run.start..position);
        run.start = position;
        run.level = level_2;
      }
      (Retention::Retained, Ordering::Equal) | (Retention::Removed, _) => {}
    },
    (_, None) => {}
  }

  Ok(())
}

impl Stack {
  fn top(&self) -> Result<Entry, Never> {
    Ok(match self.above.last() {
      Some(entry) => *entry,
      None => self.bottom,
    })
  }
}

impl Status {
  fn overriding(self) -> Result<Option<BidiClass>, Never> {
    Ok(match self {
      Status::RightToLeft => Some(BidiClass::R),
      Status::LeftToRight => Some(BidiClass::L),
      Status::Neutral | Status::Isolate => None,
    })
  }
}
