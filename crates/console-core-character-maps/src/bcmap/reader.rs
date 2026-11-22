use console_core_never::Never;
use console_core_number_conversion::index;

use crate::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Remaining {
  Something,
  Nothing,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Bytes<'a> {
  rest: &'a [u8],
}

impl<'a> Bytes<'a> {
  pub(crate) fn new(data: &'a [u8]) -> Result<Bytes<'a>, Never> {
    Ok(Bytes { rest: data })
  }

  pub(crate) fn remaining(&self) -> Result<Remaining, Never> {
    Ok(match self.rest {
      [] => Remaining::Nothing,
      [_first, ..] => Remaining::Something,
    })
  }

  pub(crate) fn u8(&mut self) -> Result<u8, ParseError> {
    match self.rest.split_first() {
      Some((first, after)) => {
        self.rest = after;

        Ok(*first)
      }
      None => Err(ParseError::Truncated),
    }
  }

  pub(crate) fn u16(&mut self) -> Result<u16, ParseError> {
    match self.rest.split_first_chunk::<2>() {
      Some((bytes, after)) => {
        self.rest = after;

        Ok(u16::from_be_bytes(*bytes))
      }
      None => Err(ParseError::Truncated),
    }
  }

  pub(crate) fn u32(&mut self) -> Result<u32, ParseError> {
    match self.rest.split_first_chunk::<4>() {
      Some((bytes, after)) => {
        self.rest = after;

        Ok(u32::from_be_bytes(*bytes))
      }
      None => Err(ParseError::Truncated),
    }
  }

  pub(crate) fn n_bytes(&mut self, width: u8) -> Result<u32, ParseError> {
    match width {
      1 => self.u8().map(u32::from),
      2 => self.u16().map(u32::from),
      3 => {
        let bytes = self.take(3)?;

        match bytes {
          [high, middle, low] => Ok(u32::from_be_bytes([0, *high, *middle, *low])),
          _other_length => Err(ParseError::Truncated),
        }
      }
      4 => self.u32(),
      _other_width => Err(ParseError::BadSegment),
    }
  }

  pub(crate) fn take(&mut self, length: u32) -> Result<&'a [u8], ParseError> {
    let Ok(at) = index(length);

    match self.rest.split_at_checked(at) {
      Some((taken, after)) => {
        self.rest = after;

        Ok(taken)
      }
      None => Err(ParseError::Truncated),
    }
  }

  pub(crate) fn terminated(&mut self) -> Result<&'a [u8], ParseError> {
    match self
      .rest
      .iter()
      .position(|byte| *byte == 0)
      .and_then(|end| self.rest.split_at_checked(end))
    {
      Some((before, [_zero, after @ ..])) => {
        self.rest = after;

        Ok(before)
      }
      Some((_, [])) | None => Err(ParseError::Truncated),
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Bits<'a> {
  rest: &'a [u8],
  used: u8,
}

impl<'a> Bits<'a> {
  pub(crate) fn new(data: &'a [u8]) -> Result<Bits<'a>, Never> {
    Ok(Bits {
      rest: data,
      used: 0,
    })
  }

  pub(crate) fn bit(&mut self) -> Result<u32, ParseError> {
    match self.rest {
      [] => Err(ParseError::Truncated),
      [byte, after @ ..] => {
        let bit = byte.rotate_left(u32::from(self.used).saturating_add(1)) & 1;

        match self.used {
          7 => {
            self.rest = after;
            self.used = 0;
          }
          _within_the_byte => self.used = self.used.saturating_add(1),
        }

        Ok(u32::from(bit))
      }
    }
  }
}
