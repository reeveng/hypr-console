//! An embedding level, the number the algorithm gives every character.
//!
//! A level runs from 0 to 126, and its parity is its direction: an even level is laid out left to
//! right and an odd one right to left. The explicit rules may nest an embedding or an isolate no
//! deeper than 125, and the implicit rules raise a level by one or two past that, so 126 is only
//! ever reached there. `Level::new` refuses a number past 126, and nothing else here makes one:
//! an explicit level that would pass 125 is an overflow the explicit rules count rather than a
//! level, and an implicit one stops at 126. Only a paragraph a caller put at 126 itself is ever
//! raised past it, and there everything stays at 126, where unicode-bidi stopped the program.
//!
//! <https://www.unicode.org/reports/tr9/#BD2>

use core::fmt;

use console_core_never::Never;

use crate::classes::BidiClass;

const DEEPEST_EXPLICIT: u8 = 125;
const DEEPEST: u8 = 126;
const ODD: u8 = 1;
const EVEN: u8 = !ODD;

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Level(u8);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Direction {
  LeftToRight,
  RightToLeft,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LevelFault {
  TooDeep { number: u8 },
}

impl fmt::Display for LevelFault {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::TooDeep { number } => write!(
        to,
        "embedding level {number} is deeper than the deepest, {DEEPEST}"
      ),
    }
  }
}

impl core::error::Error for LevelFault {}

impl Level {
  pub const LEFT_TO_RIGHT: Level = Level(0);
  pub const RIGHT_TO_LEFT: Level = Level(1);

  pub fn new(number: u8) -> Result<Level, LevelFault> {
    match number {
      ..=DEEPEST => Ok(Level(number)),
      _ => Err(LevelFault::TooDeep { number }),
    }
  }

  pub fn number(self) -> Result<u8, Never> {
    Ok(self.0)
  }

  pub fn direction(self) -> Result<Direction, Never> {
    Ok(match self.0 & ODD {
      ODD => Direction::RightToLeft,
      _ => Direction::LeftToRight,
    })
  }

  pub(crate) fn next_explicit(self, direction: Direction) -> Result<Option<Level>, Never> {
    let next = match direction {
      Direction::LeftToRight => self.0.checked_add(2).map(|number| number & EVEN),
      Direction::RightToLeft => self.0.checked_add(1).map(|number| number | ODD),
    };

    Ok(next.filter(|number| *number <= DEEPEST_EXPLICIT).map(Level))
  }

  pub(crate) fn raised(self, by: u8) -> Result<Level, Never> {
    Ok(Level(self.0.saturating_add(by).min(DEEPEST)))
  }

  pub(crate) fn strong_class(self) -> Result<BidiClass, Never> {
    let Ok(direction) = self.direction();

    Ok(match direction {
      Direction::LeftToRight => BidiClass::L,
      Direction::RightToLeft => BidiClass::R,
    })
  }
}
