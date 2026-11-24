//! The isolating run sequences (BD13), and the class at each end of one (X10).
//!
//! Without an isolate in the paragraph each level run is a sequence of its own. With one, a run
//! that ends in an isolate initiator waits for the run that starts with its matching PDI, and the
//! sequence goes on there; a sequence whose initiator is never matched is finished with whatever
//! it has when the paragraph ends. Sequences come back finished-first rather than in the order
//! they start, which the later rules do not mind.
//!
//! The class at each end is the direction of the higher of two levels: the sequence's own at that
//! end, and the level of the nearest character outside it that rule X9 does not remove, or the
//! paragraph's where there is none. After an unmatched isolate initiator the level outside is the
//! paragraph's. The characters X9 keeps are listed once, in order, so the nearest one before or
//! after any position is a binary search rather than a walk.
//!
//! <https://www.unicode.org/reports/tr9/#BD13>

use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::classes::{BidiClass, IsolateControl, Retention, retention};
use crate::level::Level;
use crate::paragraphs::Isolates;
use crate::{LevelRun, Place, at, backwards_from, forwards_from};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sequence {
  pub(crate) runs: Vec<LevelRun>,
  pub(crate) start_class: BidiClass,
  pub(crate) end_class: BidiClass,
}

struct Retained {
  positions: Vec<u32>,
}

pub(crate) struct Paragraph<'held> {
  pub(crate) level: Level,
  pub(crate) classes: &'held [BidiClass],
  pub(crate) levels: &'held [Level],
}

pub(crate) fn found(
  paragraph: &Paragraph<'_>,
  runs: Vec<LevelRun>,
  isolates: Isolates,
) -> Result<Vec<Sequence>, Never> {
  let Ok(kept) = Retained::listed(paragraph.classes);

  match isolates {
    Isolates::Absent => runs
      .into_iter()
      .map(|run| alone(paragraph, &kept, run))
      .collect(),
    Isolates::Present => {
      let Ok(joined) = joined(paragraph, &kept, runs);

      joined
        .into_iter()
        .map(|runs_2| ended(paragraph, &kept, runs_2))
        .collect()
    }
  }
}

fn alone(paragraph: &Paragraph<'_>, kept: &Retained, run: LevelRun) -> Result<Sequence, Never> {
  let Ok(first) = kept.first_from(run.start);
  let Ok(last) = kept.last_before(run.end);
  let own_start = match first.filter(|&position| position < run.end) {
    Some(position) => position,
    None => run.start,
  };
  let own_end = match last.filter(|&position| position >= run.start) {
    Some(position) => position,
    None => run.end.saturating_sub(1),
  };
  let Ok(before) = kept.last_before(run.start);
  let Ok(after) = kept.first_from(run.end);
  let Ok(start_class) = higher(paragraph, own_start, before);
  let Ok(end_class) = higher(paragraph, own_end, after);

  Ok(Sequence {
    runs: alloc::vec![run],
    start_class,
    end_class,
  })
}

fn joined(
  paragraph: &Paragraph<'_>,
  kept: &Retained,
  runs: Vec<LevelRun>,
) -> Result<Vec<Vec<LevelRun>>, Never> {
  let mut finished = Vec::with_capacity(runs.len());
  let mut waiting: Vec<Vec<LevelRun>> = Vec::new();

  for run in runs {
    let Ok(start) = at(paragraph.classes, run.start);
    let start_class = match start {
      Some(class) => class,
      None => BidiClass::BN,
    };
    let Ok(last) = kept.last_before(run.end);
    let Ok(last_class) = match last.filter(|&position| position >= run.start) {
      Some(position) => at(paragraph.classes, position),
      None => Ok(None),
    };
    let end_class = match last_class {
      Some(class) => class,
      None => start_class,
    };
    let mut sequence = match IsolateControl::from(start_class) {
      IsolateControl::Terminator => match waiting.pop() {
        Some(pending) => pending,
        None => Vec::new(),
      },
      IsolateControl::Initiator | IsolateControl::Other => Vec::new(),
    };

    sequence.push(run);

    match IsolateControl::from(end_class) {
      IsolateControl::Initiator => waiting.push(sequence),
      IsolateControl::Terminator | IsolateControl::Other => finished.push(sequence),
    }
  }

  finished.extend(waiting.into_iter().rev());

  Ok(finished)
}

fn ended(
  paragraph: &Paragraph<'_>,
  kept: &Retained,
  runs: Vec<LevelRun>,
) -> Result<Sequence, Never> {
  let start = match runs.first() {
    Some(first) => first.start,
    None => 0,
  };
  let Ok(last_run) = fitted::<_, u32>(runs.len().saturating_sub(1));
  let end = match runs.last() {
    Some(last) => last.end,
    None => 0,
  };
  let Ok(own_start) = first_retained(
    paragraph,
    &runs,
    Place {
      position: start,
      run: 0,
    },
  );
  let Ok(own_end) = last_retained(
    paragraph,
    &runs,
    Place {
      position: end,
      run: last_run,
    },
  );
  let own_start_2 = match own_start {
    Some(position) => position,
    None => start,
  };
  let own_end_2 = match own_end {
    Some(position) => position,
    None => end.saturating_sub(1),
  };
  let Ok(before) = kept.last_before(start);
  let Ok(last_kept) = kept.last_before(end);
  let Ok(last_kept_class) = match last_kept {
    Some(position) => at(paragraph.classes, position),
    None => Ok(None),
  };
  let after = match last_kept_class {
    Some(BidiClass::RLI | BidiClass::LRI | BidiClass::FSI) => None,
    Some(_) | None => {
      let Ok(first_from) = kept.first_from(end);

      first_from
    }
  };
  let Ok(start_class) = higher(paragraph, own_start_2, before);
  let Ok(end_class) = higher(paragraph, own_end_2, after);

  Ok(Sequence {
    runs,
    start_class,
    end_class,
  })
}

fn first_retained(
  paragraph: &Paragraph<'_>,
  runs: &[LevelRun],
  from: Place,
) -> Result<Option<u32>, Never> {
  let Ok(mut onwards) = forwards_from(runs, from);

  Ok(onwards.find(|&position| retention_at(paragraph, position) == Ok(Retention::Retained)))
}

fn last_retained(
  paragraph: &Paragraph<'_>,
  runs: &[LevelRun],
  from: Place,
) -> Result<Option<u32>, Never> {
  let Ok(mut backwards) = backwards_from(runs, from);

  Ok(backwards.find(|&position| retention_at(paragraph, position) == Ok(Retention::Retained)))
}

fn retention_at(paragraph: &Paragraph<'_>, position: u32) -> Result<Retention, Never> {
  let Ok(found) = at(paragraph.classes, position);

  match found {
    Some(class) => retention(class),
    None => Ok(Retention::Removed),
  }
}

fn higher(paragraph: &Paragraph<'_>, own: u32, outside: Option<u32>) -> Result<BidiClass, Never> {
  let Ok(held) = at(paragraph.levels, own);
  let own_level = match held {
    Some(level) => level,
    None => paragraph.level,
  };
  let Ok(beyond) = match outside {
    Some(position) => at(paragraph.levels, position),
    None => Ok(None),
  };
  let outside_level = match beyond {
    Some(level) => level,
    None => paragraph.level,
  };

  own_level.max(outside_level).strong_class()
}

impl Retained {
  fn listed(classes: &[BidiClass]) -> Result<Retained, Never> {
    let positions = classes
      .iter()
      .zip(0_u32..)
      .filter(|&(&class, _)| retention(class) == Ok(Retention::Retained))
      .map(|(_, position)| position)
      .collect();

    Ok(Retained { positions })
  }

  fn last_before(&self, position: u32) -> Result<Option<u32>, Never> {
    Ok(
      self
        .positions
        .partition_point(|&kept| kept < position)
        .checked_sub(1)
        .and_then(|last| self.positions.get(last))
        .copied(),
    )
  }

  fn first_from(&self, position: u32) -> Result<Option<u32>, Never> {
    Ok(
      self
        .positions
        .get(self.positions.partition_point(|&kept| kept < position))
        .copied(),
    )
  }
}
