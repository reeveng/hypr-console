//! Where a line of text may end, by Unicode Standard Annex #14.
//!
//! Given a string, `linebreaks` names the byte positions at which it may be wrapped and says
//! whether the break there is allowed or demanded, and `linebreaks_iter` does the same for
//! characters handed in one at a time, so a caller that must be able to stop part way can check
//! for that before each one it supplies. A break at the end of the text is always reported, so a
//! caller that stopped early has to look at its own input to tell that end from the real one.
//!
//! The classification is one sorted list of the code points where the line breaking class
//! changes, looked up by the last start not after the character, and the rules are the pair
//! table the annex's own algorithm reduces to: the state after the characters so far and the
//! class of the next one say whether to break and what the state becomes. The crate began as
//! unicode-linebreak, by Axel Forsman, which held the classes as a three-level trie and the
//! rest of the same table, under Unicode 15.0. The trie is gone because a trie needs bit
//! arithmetic on every lookup and this tree forbids that arithmetic unchecked; the list answers
//! the same for every code point, which is how it was made. Complex-context characters are
//! tailored to ordinary alphabetic ones whatever their general category, as upstream did, and a
//! zero width joiner is handled here rather than in the table, to keep the table small.
//!
//! A change of Unicode version replaces `tables.rs` wholesale from the annex's data, and is a
//! change of its own: some strings will break somewhere else.

mod tables;

use core::iter::FusedIterator;
use core::ops::ControlFlow;

use console_core_never::Never;
use console_core_number_conversion::index;
use tables::{CLASS_STARTS, PAIR_TABLE};

const ZERO_WIDTH_JOINER: u8 = 10;
const UNKNOWN: u8 = 42;
const END_OF_TEXT: u8 = 43;
const START_OF_TEXT: u8 = 44;

const ALLOWED_BREAK_BIT: u8 = 0x80;
const MANDATORY_BREAK_BIT: u8 = 0x40;
const BREAK_BITS: u8 = ALLOWED_BREAK_BIT | MANDATORY_BREAK_BIT;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BreakOpportunity {
  Mandatory,
  Allowed,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Class(u8);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PrecedingClass {
  Joiner,
  Other,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Row {
  after: u8,
  preceding: PrecedingClass,
}

fn class_of(character: char) -> Result<Class, Never> {
  let found = CLASS_STARTS
    .partition_point(|&(start, _)| start <= u32::from(character))
    .checked_sub(1)
    .and_then(|last| CLASS_STARTS.get(last));

  Ok(match found {
    Some(&(_, class)) => Class(class),
    None => Class(UNKNOWN),
  })
}

fn pair(row: Row, class: Class) -> Result<Option<u8>, Never> {
  let Ok(down) = index(row.after);
  let Ok(across) = index(class.0);

  Ok(
    PAIR_TABLE
      .get(down)
      .and_then(|cells| cells.get(across))
      .copied(),
  )
}

fn step<Index>(
  row: Row,
  position: Index,
  class: Class,
) -> Result<(Row, Option<(Index, BreakOpportunity)>), Never> {
  let Ok(cell) = pair(row, class);

  Ok(match cell {
    None => (row, None),
    Some(value) => {
      let opportunity = match value & BREAK_BITS {
        BREAK_BITS => Some(BreakOpportunity::Mandatory),
        ALLOWED_BREAK_BIT => match row.preceding {
          PrecedingClass::Joiner => None,
          PrecedingClass::Other => Some(BreakOpportunity::Allowed),
        },
        _ => None,
      };
      let after = Row {
        after: value & !BREAK_BITS,
        preceding: match class {
          Class(ZERO_WIDTH_JOINER) => PrecedingClass::Joiner,
          Class(_) => PrecedingClass::Other,
        },
      };

      (after, opportunity.map(|kind| (position, kind)))
    }
  })
}

#[derive(Clone, Debug)]
pub struct LineBreaks<I, Index> {
  chars: I,
  end: Index,
  progress: Progress,
}

#[derive(Clone, Copy, Debug)]
enum Progress {
  Scanning(Row),
  Finished,
}

impl<I, Index> LineBreaks<I, Index> {
  pub fn get_mut(&mut self) -> Result<&mut I, Never> {
    Ok(&mut self.chars)
  }
}

impl<I: Iterator<Item = (Index, char)>, Index: Copy> Iterator for LineBreaks<I, Index> {
  type Item = (Index, BreakOpportunity);

  fn next(&mut self) -> Option<Self::Item> {
    let initial_row = match self.progress {
      Progress::Scanning(row) => row,
      Progress::Finished => return None,
    };
    let walked = self.chars.try_fold(initial_row, |row, (position, character)| {
      let Ok(class) = class_of(character);
      let Ok((after, found)) = step(row, position, class);

      match found {
        Some(item) => ControlFlow::Break((after, item)),
        None => ControlFlow::Continue(after),
      }
    });

    match walked {
      ControlFlow::Break((after, item)) => {
        self.progress = Progress::Scanning(after);

        Some(item)
      }
      ControlFlow::Continue(last) => {
        let Ok((_after, found)) = step(last, self.end, Class(END_OF_TEXT));

        self.progress = Progress::Finished;

        found
      }
    }
  }
}

impl<I: Iterator<Item = (Index, char)>, Index: Copy> FusedIterator for LineBreaks<I, Index> {}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "a position in a str is a byte offset, which the standard library measures in usize and every caller compares against str::len"
  )
)]
pub fn linebreaks(
  s: &str,
) -> Result<impl Iterator<Item = (usize, BreakOpportunity)> + Clone + '_, Never> {
  linebreaks_iter(s.char_indices(), s.len())
}

pub fn linebreaks_iter<I: Iterator<Item = (Index, char)>, Index: Copy>(
  chars: I,
  end: Index,
) -> Result<LineBreaks<I, Index>, Never> {
  Ok(LineBreaks {
    chars,
    end,
    progress: Progress::Scanning(Row {
      after: START_OF_TEXT,
      preceding: PrecedingClass::Other,
    }),
  })
}
