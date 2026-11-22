//! The neutral and isolate formatting types (N0 to N2), resolved over one isolating run sequence.
//!
//! Paired brackets come first (N0). The pairs are found as BD16 finds them, with a stack of open
//! brackets that holds 63 at most: an opening bracket past that ends the search in the level run
//! it was found in, and a closing bracket pairs with the nearest open one of its kind and closes
//! everything opened after it. A pair whose inside holds the embedding direction takes it; one
//! whose inside holds only the other direction takes the direction of the strong character before
//! it, or of the start of the sequence; one with nothing strong inside is left alone. European
//! and Arabic numbers count as right to left there. Marks after a bracket that changed, and the
//! boundary neutrals around it, follow it.
//!
//! Then each run of neutrals takes the direction of the strong characters on both sides of it when
//! they agree, and the embedding direction when they do not (N1, N2), numbers again counting as
//! right to left, and the ends of the sequence counting as its start and end classes.
//!
//! <https://www.unicode.org/reports/tr9/#Resolving_Neutral_Types>

use alloc::vec::Vec;
use core::ops::ControlFlow;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, indices};

use crate::classes::{BidiClass, Bracket, bracket};
use crate::level::Level;
use crate::sequences::Sequence;
use crate::weak::boundary_neutrals_become;
use crate::{Place, at, backwards_from, filled, forwards_from, put};

const OPEN_BRACKETS: u32 = 63;

struct Opening {
  bracket: char,
  position: u32,
  run: u32,
}

struct Pair {
  start: u32,
  end: u32,
  start_run: u32,
  end_run: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Inside {
  Embedding,
  Opposite,
  Neither,
}

struct Between {
  before: BidiClass,
  after: BidiClass,
}

pub(crate) struct Around<'held> {
  pub(crate) levels: &'held [Level],
  pub(crate) original: &'held [BidiClass],
}

pub(crate) fn resolved(
  text: &str,
  sequence: &Sequence,
  around: &Around<'_>,
  processing: &mut [BidiClass],
) -> Result<(), Never> {
  let start = match sequence.runs.first() {
    Some(run) => run.start,
    None => 0,
  };
  let Ok(level) = at(around.levels, start);
  let Ok(embedding) = match level {
    Some(level_2) => level_2.strong_class(),
    None => Ok(BidiClass::L),
  };
  let Ok(pairs) = pairs(text, sequence, processing);

  for pair in &pairs {
    let Ok(()) = paired(text, sequence, around, processing, embedding, pair);
  }

  neutrals(sequence, embedding, processing)
}

fn pairs(text: &str, sequence: &Sequence, processing: &[BidiClass]) -> Result<Vec<Pair>, Never> {
  let mut stack: Vec<Opening> = Vec::new();
  let mut pairs = Vec::new();

  for (run_index, run) in sequence.runs.iter().enumerate() {
    let Ok(run_at) = fitted::<_, u32>(run_index);
    let here = match indices(run.clone()) {
      Ok(span) => match text.get(span) {
        Some(here) => here,
        None => "",
      },
    };

    'characters: for (offset, character) in here.char_indices() {
      let Ok(step) = fitted::<_, u32>(offset);
      let position = run.start.saturating_add(step);
      let Ok(class) = at(processing, position);
      let Ok(found) = match class {
        Some(BidiClass::ON) => bracket(character),
        Some(_) | None => Ok(None),
      };

      match found {
        Some(Bracket::Opening(bracket)) => match fitted::<_, u32>(stack.len()) {
          Ok(open) => match open {
            OPEN_BRACKETS.. => break 'characters,
            ..OPEN_BRACKETS => stack.push(Opening {
              bracket,
              position,
              run: run_at,
            }),
          },
        },
        Some(Bracket::Closing(bracket)) => {
          let Ok(closed) = closed(&mut stack, bracket);

          pairs.extend(closed.map(|opening| Pair {
            start: opening.position,
            end: position,
            start_run: opening.run,
            end_run: run_at,
          }));
        }
        None => {}
      }
    }
  }

  pairs.sort_by_key(|pair| pair.start);

  Ok(pairs)
}

fn closed(stack: &mut Vec<Opening>, bracket: char) -> Result<Option<Opening>, Never> {
  Ok(
    match stack.iter().rposition(|open| open.bracket == bracket) {
      Some(depth) => stack.drain(depth..).next(),
      None => None,
    },
  )
}

fn paired(
  text: &str,
  sequence: &Sequence,
  around: &Around<'_>,
  processing: &mut [BidiClass],
  embedding: BidiClass,
  pair: &Pair,
) -> Result<(), Never> {
  let Ok(opening_width) = width_at(text, pair.start);
  let Ok(closing_width) = width_at(text, pair.end);

  match (opening_width, closing_width) {
    (Some(start_width), Some(end_width)) => {
      let after_opening = Place {
        position: pair.start.saturating_add(start_width),
        run: pair.start_run,
      };
      let Ok(class) = chosen(sequence, processing, embedding, pair, after_opening);

      match class {
        Some(class_2) => {
          let after_closing = Place {
            position: pair.end.saturating_add(end_width),
            run: pair.end_run,
          };
          let Ok(()) = filled(processing, pair.start..after_opening.position, class_2);
          let Ok(()) = filled(processing, pair.end..after_closing.position, class_2);
          let Ok(before) = backwards_from(
            &sequence.runs,
            Place {
              position: pair.start,
              run: pair.start_run,
            },
          );
          let Ok(()) = boundary_neutrals_become(before, processing, class_2);
          let Ok(()) = marks_follow(sequence, around, processing, after_opening, class_2);

          marks_follow(sequence, around, processing, after_closing, class_2)
        }
        None => Ok(()),
      }
    }
    (_, _) => Ok(()),
  }
}

fn chosen(
  sequence: &Sequence,
  processing: &[BidiClass],
  embedding: BidiClass,
  pair: &Pair,
  after_opening: Place,
) -> Result<Option<BidiClass>, Never> {
  let Ok(enclosed) = forwards_from(&sequence.runs, after_opening);
  let inside = enclosed
    .take_while(|&position| position < pair.end)
    .try_fold(Inside::Neither, |found, position| {
      let Ok(class) = at(processing, position);

      match (strength(class, embedding), found) {
        (Ok(Inside::Embedding), _) => ControlFlow::Break(Inside::Embedding),
        (Ok(Inside::Opposite), _) => ControlFlow::Continue(Inside::Opposite),
        (Ok(Inside::Neither), held) => ControlFlow::Continue(held),
      }
    });

  match inside {
    ControlFlow::Break(_) | ControlFlow::Continue(Inside::Embedding) => Ok(Some(embedding)),
    ControlFlow::Continue(Inside::Opposite) => {
      let Ok(before) = backwards_from(
        &sequence.runs,
        Place {
          position: pair.start,
          run: pair.start_run,
        },
      );
      let strong = before
        .filter_map(|position| match at(processing, position) {
          Ok(found) => found,
        })
        .find(|&class| {
          matches!(
            class,
            BidiClass::L | BidiClass::R | BidiClass::EN | BidiClass::AN
          )
        });

      Ok(Some(match strong {
        Some(BidiClass::EN | BidiClass::AN) => BidiClass::R,
        Some(class) => class,
        None => sequence.start_class,
      }))
    }
    ControlFlow::Continue(Inside::Neither) => Ok(None),
  }
}

fn strength(class: Option<BidiClass>, embedding: BidiClass) -> Result<Inside, Never> {
  Ok(match (class, embedding) {
    (Some(BidiClass::L), BidiClass::L)
    | (Some(BidiClass::R | BidiClass::EN | BidiClass::AN), BidiClass::R) => Inside::Embedding,
    (Some(BidiClass::L), BidiClass::R)
    | (Some(BidiClass::R | BidiClass::EN | BidiClass::AN), BidiClass::L) => Inside::Opposite,
    (_, _) => Inside::Neither,
  })
}

fn marks_follow(
  sequence: &Sequence,
  around: &Around<'_>,
  processing: &mut [BidiClass],
  from: Place,
  class: BidiClass,
) -> Result<(), Never> {
  let Ok(onwards) = forwards_from(&sequence.runs, from);

  'following: for position in onwards {
    let Ok(original) = at(around.original, position);
    let Ok(now) = at(processing, position);

    match (original, now) {
      (Some(BidiClass::NSM), _) | (_, Some(BidiClass::BN)) => {
        let Ok(()) = put(processing, position, class);
      }
      (_, _) => break 'following,
    }
  }

  Ok(())
}

fn neutrals(
  sequence: &Sequence,
  embedding: BidiClass,
  processing: &mut [BidiClass],
) -> Result<(), Never> {
  let mut before = sequence.start_class;
  let mut pending: Vec<u32> = Vec::new();

  for position in sequence.runs.iter().cloned().flatten() {
    let Ok(found) = at(processing, position);

    match found {
      Some(
        BidiClass::B
        | BidiClass::S
        | BidiClass::WS
        | BidiClass::ON
        | BidiClass::FSI
        | BidiClass::LRI
        | BidiClass::RLI
        | BidiClass::PDI
        | BidiClass::BN,
      ) => pending.push(position),
      Some(class) => {
        let Ok(()) = settled(
          &mut pending,
          processing,
          Between {
            before,
            after: class,
          },
          embedding,
        );

        before = class;
      }
      None => {}
    }
  }

  settled(
    &mut pending,
    processing,
    Between {
      before,
      after: sequence.end_class,
    },
    embedding,
  )
}

fn settled(
  pending: &mut Vec<u32>,
  processing: &mut [BidiClass],
  between: Between,
  embedding: BidiClass,
) -> Result<(), Never> {
  let class = match (between.before, between.after) {
    (BidiClass::L, BidiClass::L) => BidiClass::L,
    (
      BidiClass::R | BidiClass::AN | BidiClass::EN,
      BidiClass::R | BidiClass::AN | BidiClass::EN,
    ) => BidiClass::R,
    (_, _) => embedding,
  };

  for position in pending.drain(..) {
    let Ok(()) = put(processing, position, class);
  }

  Ok(())
}

fn width_at(text: &str, position: u32) -> Result<Option<u32>, Never> {
  let Ok(start) = index(position);

  Ok(
    match text.get(start..).and_then(|rest| rest.chars().next()) {
      Some(character) => match fitted::<_, u32>(character.len_utf8()) {
        Ok(width) => Some(width),
      },
      None => None,
    },
  )
}
