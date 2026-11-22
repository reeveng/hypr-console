//! The weak types (W1 to W7), resolved over one isolating run sequence.
//!
//! The annex writes these as seven passes, one after another over the whole sequence. They are
//! one pass here, a byte at a time, which is simple for the rules that read only the character
//! itself and needs a little memory for the rest: the class before W1 changed anything, for the
//! mark that follows it; the class before W4, for the separator between two numbers; the class
//! before W5, for a terminator beside a number; whether the last strong character was Arabic; and
//! the terminators and boundary neutrals waiting to learn whether a European number follows them.
//! W7 is a second pass, because it reads what the first one left.
//!
//! Boundary neutrals are skipped wherever the annex reads a neighbour, as its section on retaining
//! the formatting characters asks, and are given the class of what surrounds them where it says.
//! A separator in the middle of a character takes the class its first byte was given.
//!
//! <https://www.unicode.org/reports/tr9/#Resolving_Weak_Types>

use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::classes::{BidiClass, IsolateControl, Retention, retention};
use crate::sequences::Sequence;
use crate::{Place, at, backwards_from, forwards_from, put};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LastStrong {
  Arabic,
  Left,
  Right,
}

struct Weak {
  before_w1: BidiClass,
  before_w4: BidiClass,
  before_w5: BidiClass,
  last_strong: LastStrong,
  terminators: Vec<u32>,
  boundary_neutrals: Vec<u32>,
}

pub(crate) fn resolved(
  text: &str,
  sequence: &Sequence,
  processing: &mut [BidiClass],
) -> Result<(), Never> {
  let mut weak = Weak {
    before_w1: sequence.start_class,
    before_w4: sequence.start_class,
    before_w5: sequence.start_class,
    last_strong: LastStrong::Right,
    terminators: Vec::new(),
    boundary_neutrals: Vec::new(),
  };

  for (run_index, run) in sequence.runs.iter().enumerate() {
    let Ok(run_at) = fitted::<_, u32>(run_index);

    for position in run.clone() {
      let Ok(found) = at(processing, position);

      match found {
        Some(BidiClass::BN) => weak.boundary_neutrals.push(position),
        Some(_) => {
          let Ok(()) = weak.stepped(
            text,
            sequence,
            Place {
              position,
              run: run_at,
            },
            processing,
          );
        }
        None => {}
      }
    }
  }

  let Ok(()) = every(&mut weak.terminators, processing, BidiClass::ON);

  europeans_after_left(sequence, processing)
}

impl Weak {
  fn stepped(
    &mut self,
    text: &str,
    sequence: &Sequence,
    place: Place,
    processing: &mut [BidiClass],
  ) -> Result<(), Never> {
    let Ok(found) = at(processing, place.position);
    let class = match found {
      Some(class) => class,
      None => BidiClass::BN,
    };
    let Ok(before_w456) = self.marked(class);
    let Ok(()) = put(processing, place.position, before_w456);

    match before_w456 {
      BidiClass::EN => {
        let Ok(()) = every(&mut self.terminators, processing, BidiClass::EN);
      }
      BidiClass::ES | BidiClass::CS => {
        let Ok(()) = self.separated(text, sequence, place, before_w456, processing);
      }
      BidiClass::ET => match self.before_w5 {
        BidiClass::EN => {
          let Ok(()) = put(processing, place.position, BidiClass::EN);
        }
        BidiClass::AL
        | BidiClass::AN
        | BidiClass::B
        | BidiClass::BN
        | BidiClass::CS
        | BidiClass::ES
        | BidiClass::ET
        | BidiClass::FSI
        | BidiClass::L
        | BidiClass::LRE
        | BidiClass::LRI
        | BidiClass::LRO
        | BidiClass::NSM
        | BidiClass::ON
        | BidiClass::PDF
        | BidiClass::PDI
        | BidiClass::R
        | BidiClass::RLE
        | BidiClass::RLI
        | BidiClass::RLO
        | BidiClass::S
        | BidiClass::WS => {
          self
            .terminators
            .extend(self.boundary_neutrals.iter().copied());
          self.terminators.push(place.position);
        }
      },
      BidiClass::AL
      | BidiClass::AN
      | BidiClass::B
      | BidiClass::BN
      | BidiClass::FSI
      | BidiClass::L
      | BidiClass::LRE
      | BidiClass::LRI
      | BidiClass::LRO
      | BidiClass::NSM
      | BidiClass::ON
      | BidiClass::PDF
      | BidiClass::PDI
      | BidiClass::R
      | BidiClass::RLE
      | BidiClass::RLI
      | BidiClass::RLO
      | BidiClass::S
      | BidiClass::WS => {}
    }

    self.boundary_neutrals.clear();

    let Ok(now) = at(processing, place.position);

    self.before_w5 = match now {
      Some(class_2) => class_2,
      None => before_w456,
    };

    match self.before_w5 {
      BidiClass::ET => {}
      BidiClass::AL
      | BidiClass::AN
      | BidiClass::B
      | BidiClass::BN
      | BidiClass::CS
      | BidiClass::EN
      | BidiClass::ES
      | BidiClass::FSI
      | BidiClass::L
      | BidiClass::LRE
      | BidiClass::LRI
      | BidiClass::LRO
      | BidiClass::NSM
      | BidiClass::ON
      | BidiClass::PDF
      | BidiClass::PDI
      | BidiClass::R
      | BidiClass::RLE
      | BidiClass::RLI
      | BidiClass::RLO
      | BidiClass::S
      | BidiClass::WS => {
        let Ok(()) = every(&mut self.terminators, processing, BidiClass::ON);
      }
    }

    self.before_w4 = before_w456;

    Ok(())
  }

  fn marked(&mut self, class: BidiClass) -> Result<BidiClass, Never> {
    let after_w1 = match (class, IsolateControl::from(self.before_w1)) {
      (BidiClass::NSM, IsolateControl::Initiator | IsolateControl::Terminator) => BidiClass::ON,
      (BidiClass::NSM, IsolateControl::Other) => self.before_w1,
      (unchanged, _) => unchanged,
    };
    let after_w3 = match (after_w1, self.last_strong) {
      (BidiClass::EN, LastStrong::Arabic) => BidiClass::AN,
      (BidiClass::AL, _) => BidiClass::R,
      (unchanged, _) => unchanged,
    };

    self.before_w1 = after_w1;
    self.last_strong = match (after_w1, self.last_strong) {
      (BidiClass::L, _) => LastStrong::Left,
      (BidiClass::R, _) => LastStrong::Right,
      (BidiClass::AL, _) => LastStrong::Arabic,
      (_, held) => held,
    };

    Ok(after_w3)
  }

  fn separated(
    &mut self,
    text: &str,
    sequence: &Sequence,
    place: Place,
    separator: BidiClass,
    processing: &mut [BidiClass],
  ) -> Result<(), Never> {
    let Ok(start) = index(place.position);

    match text.get(start..).and_then(|rest| rest.chars().next()) {
      Some(character) => {
        let Ok(width) = fitted::<_, u32>(character.len_utf8());
        let after = Place {
          position: place.position.saturating_add(width),
          run: place.run,
        };
        let Ok(next_class) = next_retained(sequence, after, processing);
        let next_class_2 = match (next_class, self.last_strong) {
          (BidiClass::EN, LastStrong::Arabic) => BidiClass::AN,
          (unchanged, _) => unchanged,
        };
        let resolved = match (self.before_w4, separator, next_class_2) {
          (BidiClass::EN, BidiClass::ES | BidiClass::CS, BidiClass::EN) => BidiClass::EN,
          (BidiClass::AN, BidiClass::CS, BidiClass::AN) => BidiClass::AN,
          (_, _, _) => BidiClass::ON,
        };
        let Ok(()) = put(processing, place.position, resolved);

        match resolved {
          BidiClass::ON => {
            let Ok(before) = backwards_from(&sequence.runs, place);
            let Ok(onwards) = forwards_from(&sequence.runs, after);
            let Ok(()) = boundary_neutrals_become(before, processing, BidiClass::ON);

            boundary_neutrals_become(onwards, processing, BidiClass::ON)
          }
          BidiClass::AL
          | BidiClass::AN
          | BidiClass::B
          | BidiClass::BN
          | BidiClass::CS
          | BidiClass::EN
          | BidiClass::ES
          | BidiClass::ET
          | BidiClass::FSI
          | BidiClass::L
          | BidiClass::LRE
          | BidiClass::LRI
          | BidiClass::LRO
          | BidiClass::NSM
          | BidiClass::PDF
          | BidiClass::PDI
          | BidiClass::R
          | BidiClass::RLE
          | BidiClass::RLI
          | BidiClass::RLO
          | BidiClass::S
          | BidiClass::WS => Ok(()),
        }
      }
      None => {
        let Ok(previous) = at(processing, place.position.saturating_sub(1));

        match previous {
          Some(class) => put(processing, place.position, class),
          None => Ok(()),
        }
      }
    }
  }
}

fn next_retained(
  sequence: &Sequence,
  after: Place,
  processing: &[BidiClass],
) -> Result<BidiClass, Never> {
  let Ok(onwards) = forwards_from(&sequence.runs, after);
  let found = onwards
    .filter_map(|position| match at(processing, position) {
      Ok(found) => found,
    })
    .find(|&class| retention(class) == Ok(Retention::Retained));

  Ok(match found {
    Some(class) => class,
    None => sequence.end_class,
  })
}

pub(crate) fn boundary_neutrals_become(
  positions: impl Iterator<Item = u32>,
  processing: &mut [BidiClass],
  class: BidiClass,
) -> Result<(), Never> {
  'spreading: for position in positions {
    let Ok(at_position) = index(position);

    match processing.get_mut(at_position) {
      Some(held @ BidiClass::BN) => *held = class,
      Some(_) | None => break 'spreading,
    }
  }

  Ok(())
}

fn every(
  positions: &mut Vec<u32>,
  processing: &mut [BidiClass],
  class: BidiClass,
) -> Result<(), Never> {
  for position in positions.drain(..) {
    let Ok(()) = put(processing, position, class);
  }

  Ok(())
}

fn europeans_after_left(sequence: &Sequence, processing: &mut [BidiClass]) -> Result<(), Never> {
  let mut last_strong = match sequence.start_class {
    BidiClass::L => LastStrong::Left,
    BidiClass::AL
    | BidiClass::AN
    | BidiClass::B
    | BidiClass::BN
    | BidiClass::CS
    | BidiClass::EN
    | BidiClass::ES
    | BidiClass::ET
    | BidiClass::FSI
    | BidiClass::LRE
    | BidiClass::LRI
    | BidiClass::LRO
    | BidiClass::NSM
    | BidiClass::ON
    | BidiClass::PDF
    | BidiClass::PDI
    | BidiClass::R
    | BidiClass::RLE
    | BidiClass::RLI
    | BidiClass::RLO
    | BidiClass::S
    | BidiClass::WS => LastStrong::Right,
  };

  for position in sequence.runs.iter().cloned().flatten() {
    let Ok(found) = at(processing, position);

    match (found, last_strong) {
      (Some(BidiClass::EN), LastStrong::Left) => {
        let Ok(()) = put(processing, position, BidiClass::L);
      }
      (Some(BidiClass::L), _) => last_strong = LastStrong::Left,
      (Some(BidiClass::R | BidiClass::AL), _) => last_strong = LastStrong::Right,
      (_, _) => {}
    }
  }

  Ok(())
}
