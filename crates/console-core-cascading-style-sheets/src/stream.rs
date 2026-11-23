use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::Error;

const SPACES: [char; 5] = [' ', '\t', '\n', '\r', '\x0C'];

pub(crate) const NAME_START: fn(char) -> bool =
  |c| matches!(c, '_' | 'a'..='z' | 'A'..='Z') || u32::from(c) > 237;

pub(crate) const NAME: fn(char) -> bool =
  |c| matches!(c, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9' | '-') || u32::from(c) > 237;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Consumption {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Stream<'a> {
  rest: &'a str,
}

impl<'a> From<&'a str> for Stream<'a> {
  fn from(text: &'a str) -> Stream<'a> {
    Stream { rest: text }
  }
}

impl<'a> Stream<'a> {
  pub(crate) fn first(&self) -> Result<Option<u8>, Never> {
    Ok(self.rest.bytes().next())
  }

  pub(crate) fn second(&self) -> Result<Option<u8>, Never> {
    Ok(self.rest.as_bytes().get(1).copied())
  }

  pub(crate) fn budget(&self) -> Result<u32, Never> {
    fitted::<_, u32>(self.rest.len())
  }

  pub(crate) fn bump(&mut self) -> Result<(), Never> {
    let mut chars = self.rest.chars();
    let _taken = chars.next();

    self.rest = chars.as_str();

    Ok(())
  }

  pub(crate) fn exhaust(&mut self) -> Result<(), Never> {
    self.rest = "";

    Ok(())
  }

  pub(crate) fn since(&self, earlier: Stream<'a>) -> Result<&'a str, Error> {
    match earlier
      .rest
      .split_at_checked(earlier.rest.len().saturating_sub(self.rest.len()))
    {
      Some((walked, _rest)) => Ok(walked),
      None => Err(Error::Misaligned),
    }
  }

  pub(crate) fn skip_spaces(&mut self) -> Result<(), Never> {
    self.rest = self.rest.trim_start_matches(SPACES);

    Ok(())
  }

  pub(crate) fn skip_while(&mut self, keep: impl Fn(char) -> bool) -> Result<(), Never> {
    self.rest = self.rest.trim_start_matches(keep);

    Ok(())
  }

  pub(crate) fn try_consume(&mut self, text: &str) -> Result<Consumption, Never> {
    Ok(match self.rest.strip_prefix(text) {
      Some(tail) => {
        self.rest = tail;
        Consumption::Yes
      }
      None => Consumption::No,
    })
  }

  pub(crate) fn consume(&mut self, text: &str) -> Result<(), Error> {
    match self.rest.strip_prefix(text) {
      Some(tail) => {
        self.rest = tail;
        Ok(())
      }
      None => match self.rest.chars().next() {
        Some(_other) => Err(Error::InvalidByte),
        None => Err(Error::UnexpectedEnd),
      },
    }
  }

  pub(crate) fn take_until(&mut self, stop: char) -> Result<&'a str, Error> {
    let start = *self;

    self.rest = self.rest.trim_start_matches(|c| c != stop);

    self.since(start)
  }

  pub(crate) fn skip_comment(&mut self) -> Result<(), Error> {
    self.consume("/").map_err(|_fault| Error::InvalidComment)?;
    self.consume("*").map_err(|_fault| Error::InvalidComment)?;

    match self.rest.split_once("*/") {
      Some((_comment, tail)) => {
        self.rest = tail;
        Ok(())
      }
      None => {
        self.rest = "";
        Err(Error::InvalidComment)
      }
    }
  }

  pub(crate) fn skip_spaces_and_comments(&mut self) -> Result<(), Error> {
    self.skip_spaces()?;

    let bound = self.budget()?;

    for _comment in 0..=bound {
      match self.rest.get(..2) {
        Some("/*") => {
          self.skip_comment()?;
          self.skip_spaces()?;
        }
        Some(_) | None => return Ok(()),
      }
    }

    Ok(())
  }

  pub(crate) fn skip_block(&mut self) -> Result<(), Never> {
    let mut chars = self.rest.chars();
    let mut depth = 0u32;

    let _closing = chars.by_ref().find(move |c| match c {
      '{' => {
        depth = depth.saturating_add(1);
        false
      }
      '}' => match depth {
        0 => true,
        _nested => {
          depth = depth.saturating_sub(1);
          false
        }
      },
      _other => false,
    });

    self.rest = chars.as_str();

    Ok(())
  }

  pub(crate) fn consume_ident(&mut self) -> Result<&'a str, Error> {
    let start = *self;
    let dash = self.try_consume("-")?;

    match self.rest.chars().next() {
      Some(first) => match ::core::primitive::bool::then_some(NAME_START(first), ()) {
        ::core::option::Option::Some(()) => {
          self.bump()?;
        }
        ::core::option::Option::None => return Err(Error::InvalidIdent),
      },
      None => match dash {
        Consumption::Yes => {}
        Consumption::No => return Err(Error::InvalidIdent),
      },
    }

    self.skip_while(NAME)?;

    self.since(start)
  }

  pub(crate) fn consume_string(&mut self) -> Result<&'a str, Error> {
    let quote = match self.rest.chars().next() {
      Some(quote @ ('\'' | '"')) => quote,
      Some(_other) => return self.consume_ident(),
      None => return Err(Error::UnexpectedEnd),
    };

    self.bump()?;

    let body = *self;
    let mut chars = self.rest.chars();
    let mut previous = quote;

    let closing = chars.by_ref().find(move |c| {
      let closes = *c == quote && previous != '\\';

      previous = *c;

      closes
    });

    match closing {
      Some(_quote) => {
        self.rest = chars.as_str();

        let walked = self.since(body)?;

        walked.strip_suffix(quote).ok_or(Error::Misaligned)
      }
      None => {
        self.rest = "";
        Err(Error::UnexpectedEnd)
      }
    }
  }
}
