//! The implicit levels (I1, I2), and the levels of what rule X9 removed.
//!
//! A character on a left-to-right level goes up one for a right-to-left class and two for a
//! number; one on a right-to-left level goes up one for a left-to-right class or a number. The
//! characters X9 removed are given no level by the annex, so each takes the level of the character
//! before it, or the paragraph's at the start, which keeps them from breaking a level run apart.
//!
//! <https://www.unicode.org/reports/tr9/#Resolving_Implicit_Levels>

use console_core_never::Never;

use crate::classes::{BidiClass, Retention, retention};
use crate::level::{Direction, Level};

pub(crate) fn resolved(processing: &[BidiClass], levels: &mut [Level]) -> Result<(), Never> {
  for (level, &class) in levels.iter_mut().zip(processing) {
    let Ok(direction) = level.direction();
    let Ok(raised) = match (direction, class) {
      (Direction::LeftToRight, BidiClass::AN | BidiClass::EN) => level.raised(2),
      (Direction::LeftToRight, BidiClass::R)
      | (Direction::RightToLeft, BidiClass::L | BidiClass::EN | BidiClass::AN) => level.raised(1),
      (_, _) => Ok(*level),
    };

    *level = raised;
  }

  Ok(())
}

pub(crate) fn removed_follow(
  paragraph_level: Level,
  classes: &[BidiClass],
  levels: &mut [Level],
) -> Result<(), Never> {
  let mut previous = paragraph_level;

  for (level, &class) in levels.iter_mut().zip(classes) {
    let Ok(kept) = retention(class);

    match kept {
      Retention::Removed => *level = previous,
      Retention::Retained => {}
    }

    previous = *level;
  }

  Ok(())
}
