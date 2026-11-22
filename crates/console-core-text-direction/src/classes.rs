//! What the algorithm asks of a character: its Bidi_Class, and whether it is a paired bracket.
//!
//! The class of a character is the range in `tables::BIDI_CLASSES` it falls in, found by the last
//! range that starts at or before it; a character after the end of that range is in none, and is
//! L. A bracket is answered with the opening bracket of its pair, canonically equivalent spellings
//! folded onto one, so an opening and a closing bracket pair when their answers are the same
//! character. The groupings the rules keep asking about -- which classes rule X9 removes, which
//! are neutral or isolate formatting -- are named here once rather than spelled at each rule.

use core::cmp::Ordering;

use console_core_never::Never;

use crate::tables::{BIDI_CLASSES, BRACKET_PAIRS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BidiClass {
  AL,
  AN,
  B,
  BN,
  CS,
  EN,
  ES,
  ET,
  FSI,
  L,
  LRE,
  LRI,
  LRO,
  NSM,
  ON,
  PDF,
  PDI,
  R,
  RLE,
  RLI,
  RLO,
  S,
  WS,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bracket {
  Opening(char),
  Closing(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retention {
  Removed,
  Retained,
}

pub(crate) enum IsolateControl {
  Initiator,
  Terminator,
  Other,
}

impl From<BidiClass> for IsolateControl {
  fn from(class: BidiClass) -> Self {
    match class {
      BidiClass::FSI | BidiClass::LRI | BidiClass::RLI => Self::Initiator,
      BidiClass::PDI => Self::Terminator,
      BidiClass::AL
      | BidiClass::AN
      | BidiClass::B
      | BidiClass::BN
      | BidiClass::CS
      | BidiClass::EN
      | BidiClass::ES
      | BidiClass::ET
      | BidiClass::L
      | BidiClass::LRE
      | BidiClass::LRO
      | BidiClass::NSM
      | BidiClass::ON
      | BidiClass::PDF
      | BidiClass::R
      | BidiClass::RLE
      | BidiClass::RLO
      | BidiClass::S
      | BidiClass::WS => Self::Other,
    }
  }
}

pub fn bidi_class(character: char) -> Result<BidiClass, Never> {
  let found = BIDI_CLASSES
    .partition_point(|&(first, _, _)| first <= character)
    .checked_sub(1)
    .and_then(|last| BIDI_CLASSES.get(last));

  Ok(match found {
    Some(&(_, last, class)) => match character.cmp(&last) {
      Ordering::Less | Ordering::Equal => class,
      Ordering::Greater => BidiClass::L,
    },
    None => BidiClass::L,
  })
}

pub(crate) fn bracket(character: char) -> Result<Option<Bracket>, Never> {
  Ok(
    BRACKET_PAIRS
      .iter()
      .find_map(|&(opening, closing, equivalent)| {
        let folded = match equivalent {
          Some(held) => held,
          None => opening,
        };

        match (character.cmp(&opening), character.cmp(&closing)) {
          (Ordering::Equal, _) => Some(Bracket::Opening(folded)),
          (_, Ordering::Equal) => Some(Bracket::Closing(folded)),
          (_, _) => None,
        }
      }),
  )
}

pub(crate) fn retention(class: BidiClass) -> Result<Retention, Never> {
  Ok(match class {
    BidiClass::RLE
    | BidiClass::LRE
    | BidiClass::RLO
    | BidiClass::LRO
    | BidiClass::PDF
    | BidiClass::BN => Retention::Removed,
    BidiClass::AL
    | BidiClass::AN
    | BidiClass::B
    | BidiClass::CS
    | BidiClass::EN
    | BidiClass::ES
    | BidiClass::ET
    | BidiClass::FSI
    | BidiClass::L
    | BidiClass::LRI
    | BidiClass::NSM
    | BidiClass::ON
    | BidiClass::PDI
    | BidiClass::R
    | BidiClass::RLI
    | BidiClass::S
    | BidiClass::WS => Retention::Retained,
  })
}
