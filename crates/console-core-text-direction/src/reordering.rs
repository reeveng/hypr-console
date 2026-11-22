//! A line's levels made ready for display (L1), and the order it is shown in (L2).
//!
//! L1 puts the paragraph's level back on the separators, on the whitespace and isolate formatting
//! before them and at the end of the line, and the formatting characters X9 removed take the level
//! of what is before them, as the annex's section on retaining them asks. L2 reverses, from the
//! highest level on the line down to the lowest odd one, every stretch of the line at that level
//! or higher. A stretch is found by the levels of what is in it, so it is the same stretch whether
//! the levels are read where they started or where the reversals so far have moved them: what
//! moved was all at a higher level than anything still to be decided.
//!
//! The lowest odd level is the line's lowest level made odd, and is counted as a number rather
//! than a level, so a line all at 126 has no odd level of its own to stop at and is not reversed,
//! which is what the reference implementation does with it.
//!
//! <https://www.unicode.org/reports/tr9/#Reordering_Resolved_Levels>

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{fitted, indices};

use crate::classes::BidiClass;
use crate::level::Level;
use crate::{LevelRun, at, filled};

const ODD: u8 = 1;

pub(crate) struct Line<'held> {
  pub(crate) text: &'held str,
  pub(crate) range: Range<u32>,
  pub(crate) paragraph_level: Level,
}

pub(crate) fn line_levels(
  classes: &[BidiClass],
  levels: &mut [Level],
  line: &Line<'_>,
) -> Result<(), Never> {
  let mut reset_from = Some(line.range.start);
  let mut previous = line.paragraph_level;

  for (offset, character) in line.text.char_indices() {
    let Ok(step) = fitted::<_, u32>(offset);
    let Ok(width) = fitted::<_, u32>(character.len_utf8());
    let position = line.range.start.saturating_add(step);
    let after = position.saturating_add(width);
    let Ok(found) = at(classes, position);

    match found {
      Some(BidiClass::B | BidiClass::S) => {
        let from = match reset_from {
          Some(from) => from,
          None => position,
        };
        let Ok(()) = filled(levels, from..after, line.paragraph_level);

        reset_from = None;
      }
      Some(BidiClass::WS | BidiClass::FSI | BidiClass::LRI | BidiClass::RLI | BidiClass::PDI) => {
        reset_from = reset_from.or(Some(position));
      }
      Some(
        BidiClass::RLE
        | BidiClass::LRE
        | BidiClass::RLO
        | BidiClass::LRO
        | BidiClass::PDF
        | BidiClass::BN,
      ) => {
        reset_from = reset_from.or(Some(position));

        let Ok(()) = filled(levels, position..after, previous);
      }
      Some(
        BidiClass::L
        | BidiClass::R
        | BidiClass::AL
        | BidiClass::EN
        | BidiClass::ES
        | BidiClass::ET
        | BidiClass::AN
        | BidiClass::CS
        | BidiClass::NSM
        | BidiClass::ON,
      ) => reset_from = None,
      None => {}
    }

    let Ok(now) = at(levels, position);

    match now {
      Some(level) => previous = level,
      None => {}
    }
  }

  match reset_from {
    Some(from) => filled(levels, from..line.range.end, line.paragraph_level),
    None => Ok(()),
  }
}

pub(crate) fn runs(levels: &[Level], line: Range<u32>) -> Result<Vec<LevelRun>, Never> {
  let Ok(first) = at(levels, line.start);

  Ok(match first {
    Some(first_2) => {
      let mut found: Vec<(LevelRun, u8)> = Vec::new();
      let mut run = (line.start, first_2);
      let within = match indices(line.clone()) {
        Ok(span) => match levels.get(span) {
          Some(within) => within,
          None => &[],
        },
      };

      for (position, &level) in (line.start..).zip(within).skip(1) {
        match level.cmp(&run.1) {
          Ordering::Equal => {}
          Ordering::Less | Ordering::Greater => {
            let Ok(depth) = run.1.number();

            found.push((run.0..position, depth));
            run = (position, level);
          }
        }
      }

      let Ok(depth) = run.1.number();

      found.push((run.0..line.end, depth));

      let Ok(()) = reversed(&mut found);

      found.into_iter().map(|(run_2, _)| run_2).collect()
    }
    None => Vec::new(),
  })
}

pub(crate) fn visual_order(levels: &[Level]) -> Result<Vec<u32>, Never> {
  let mut found: Vec<(u32, u8)> = (0_u32..)
    .zip(levels)
    .map(|(position, level)| match level.number() {
      Ok(depth) => (position, depth),
    })
    .collect();
  let Ok(()) = reversed(&mut found);

  Ok(found.into_iter().map(|(position, _)| position).collect())
}

fn reversed<T>(held: &mut [(T, u8)]) -> Result<(), Never> {
  let lowest = held.iter().map(|&(_, depth)| depth).min();
  let highest = held.iter().map(|&(_, depth)| depth).max();

  let (first_odd, deepest) = match (lowest, highest) {
    (Some(shallowest), Some(deepest)) => (shallowest | ODD, deepest),
    (_, _) => return Ok(()),
  };

  for depth in (first_odd..=deepest).rev() {
    for stretch in held.chunk_by_mut(|left, right| (left.1 >= depth) == (right.1 >= depth)) {
      match stretch.first().map(|&(_, held_2)| held_2.cmp(&depth)) {
        Some(Ordering::Equal | Ordering::Greater) => stretch.reverse(),
        Some(Ordering::Less) | None => {}
      }
    }
  }

  Ok(())
}
