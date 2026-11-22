//! A date as ID3v2.4 writes one, read the forgiving way lofty read it.
//!
//! A year of four characters comes first and the rest is optional: a month,
//! a day, an hour, a minute and a second, each two characters, separated by
//! a dash or a dot within the date, a `T` before the hour and a colon within
//! the time -- except that a text with no separator anywhere is read as its
//! digits run together. Spaces count as characters and add nothing, a field
//! that cannot be read ends the date where it is rather than refusing it,
//! and only a character that belongs to no date at all is a fault. A date
//! that reads but names a month thirteen is kept, and only becomes an item
//! once it is checked.

use std::fmt;

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::bytes::Cursor;
use crate::fault::Malformed;
use crate::id3v2::Validity;

const LONGEST: u64 = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Timestamp {
  pub(crate) year: u16,
  pub(crate) month: Option<u8>,
  pub(crate) day: Option<u8>,
  pub(crate) hour: Option<u8>,
  pub(crate) minute: Option<u8>,
  pub(crate) second: Option<u8>,
}

enum Class {
  Space,
  Digit(u8),
  Separator,
  Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Acceptance {
  Accepted,
  Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Before {
  Nothing,
  DateSeparator,
  TimeMark,
  TimeSeparator,
}

enum Layout {
  Delimited,
  Compact,
}

impl From<bool> for Layout {
  fn from(separated: bool) -> Self {
    match bool::then_some(separated, ()) {
      Some(()) => Self::Delimited,
      None => Self::Compact,
    }
  }
}

fn classed(byte: u8) -> Result<Class, Never> {
  Ok(match byte {
    b' ' => Class::Space,
    digit @ b'0'..=b'9' => Class::Digit(digit),
    b'-' | b'.' | b'T' | b':' => Class::Separator,
    _other => Class::Other,
  })
}

fn accepted(before: Before, byte: u8) -> Result<Acceptance, Never> {
  Ok(match (before, byte) {
    (Before::DateSeparator, b'-' | b'.')
    | (Before::TimeMark, b'T')
    | (Before::TimeSeparator, b':') => Acceptance::Accepted,
    (Before::Nothing | Before::DateSeparator | Before::TimeMark | Before::TimeSeparator, _) => {
      Acceptance::Refused
    }
  })
}

fn segment(reader: &mut Cursor<'_>, size: u64, before: Before) -> Result<Option<u16>, Malformed> {
  let Ok(left) = reader.left();

  match left {
    0 => return Ok(None),
    _left => {}
  }

  match before {
    Before::Nothing => {}
    Before::DateSeparator | Before::TimeMark | Before::TimeSeparator => {
      let byte = reader.byte()?;
      let Ok(acceptance) = accepted(before, byte);

      match acceptance {
        Acceptance::Accepted => {}
        Acceptance::Refused => return Ok(None),
      }
    }
  }

  let Ok(rest) = reader.rest();
  let Ok(mut peek) = Cursor::new(rest);

  let digits = match peek.take(size) {
    Ok(digits) => digits,
    Err(_not_enough_digits) => return Ok(None),
  };

  let mut number: Option<u16> = None;
  let mut counted = 0u64;

  'digits: for byte in digits {
    let Ok(class) = classed(*byte);

    match class {
      Class::Space => counted = counted.saturating_add(1),
      Class::Digit(digit) => {
        let value = u16::from(digit.saturating_sub(b'0'));

        let held = match number {
          Some(held) => held,
          None => 0,
        };

        number = Some(held.saturating_mul(10).saturating_add(value));
        counted = counted.saturating_add(1);
      }
      Class::Separator => match before {
        Before::DateSeparator | Before::TimeMark | Before::TimeSeparator => break 'digits,
        Before::Nothing => return Err(Malformed::Timestamp),
      },
      Class::Other => return Err(Malformed::Timestamp),
    }
  }

  match number {
    Some(number_2) => {
      reader.skip(counted)?;

      Ok(Some(number_2))
    }
    None => Ok(None),
  }
}

impl Timestamp {
  pub(crate) fn parse(written: &[u8]) -> Result<Option<Timestamp>, Malformed> {
    match written {
      [] => return Ok(None),
      [_, ..] => {}
    }

    let trimmed = written.trim_ascii_start();

    match trimmed {
      [] => return Err(Malformed::Timestamp),
      [_, ..] => {}
    }

    let Ok(mut reader) = Cursor::new(trimmed);
    let Ok(content) = reader.upto(LONGEST);

    Timestamp::parsed(content)
  }

  fn parsed(content: &[u8]) -> Result<Option<Timestamp>, Malformed> {
    let layout = Layout::from(
      content
        .iter()
        .any(|byte| matches!(byte, b'-' | b'.' | b':')),
    );
    let Ok(mut reader) = Cursor::new(content);

    let Ok(before) = reader.left();
    let year = segment(&mut reader, 4, Before::Nothing)?;
    let Ok(after) = reader.left();

    let year_3 = match (year, before.saturating_sub(after)) {
      (Some(year_2), 4) => year_2,
      (Some(_), _) | (None, _) => return Ok(None),
    };

    let (date, clock) = match layout {
      Layout::Delimited => (Before::DateSeparator, Before::TimeSeparator),
      Layout::Compact => (Before::Nothing, Before::Nothing),
    };

    let mut fields: [Option<u8>; 5] = [None; 5];

    'fields: for (field, before_2) in
      fields
        .iter_mut()
        .zip([date, date, Before::TimeMark, clock, clock])
    {
      let value = segment(&mut reader, 2, before_2)?;

      match value {
        Some(value_2) => {
          let Ok(value_3) = fitted::<u16, u8>(value_2);

          *field = Some(value_3);
        }
        None => break 'fields,
      }
    }

    let [month, day, hour, minute, second] = fields;

    Ok(Some(Timestamp {
      year: year_3,
      month,
      day,
      hour,
      minute,
      second,
    }))
  }

  pub(crate) fn verified(&self) -> Result<Validity, Never> {
    let limits = [
      (self.month, 12, Some(1)),
      (self.day, 31, self.month),
      (self.hour, 23, self.day),
      (self.minute, 59, self.hour),
      (self.second, 59, self.minute),
    ];

    let fields = limits
      .iter()
      .all(|(field, limit, parent)| match (field, parent) {
        (None, _parent) => true,
        (Some(value), Some(_parent)) => value <= limit,
        (Some(_value), None) => false,
      });

    Ok(Validity::from(self.year <= 9999 && fields))
  }
}

impl fmt::Display for Timestamp {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "{:04}", self.year)?;

    let parts = [
      ("-", self.month),
      ("-", self.day),
      ("T", self.hour),
      (":", self.minute),
      (":", self.second),
    ];

    'parts: for (mark, part) in parts {
      match part {
        Some(value) => {
          write!(to, "{mark}{value:02}")?;
        }
        None => break 'parts,
      }
    }

    Ok(())
  }
}
