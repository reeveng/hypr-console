//! What a byte is to the scanner: white space, a delimiter or part of a
//! token, and whether it ends a line, which is where a comment stops.
//!
//! These are PostScript's own classes, which PDF took over unchanged, and
//! every walk in the crate asks them here. A walk takes bytes off the front of
//! a slice and hands back what is left, so a part of a program is always the
//! bytes walked over before the rest began, and [`Walk`] is that question.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
  WhiteSpace,
  Delimiter,
  Regular,
}

pub(crate) fn class(byte: u8) -> Result<Class, Never> {
  Ok(match byte {
    0x00 | 0x09 | 0x0a | 0x0c | 0x0d | 0x20 => Class::WhiteSpace,
    b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%' => Class::Delimiter,
    _regular => Class::Regular,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Line {
  Ends,
  Continues,
}

pub(crate) fn line(byte: u8) -> Result<Line, Never> {
  Ok(match byte {
    0x0a | 0x0d => Line::Ends,
    _within => Line::Continues,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follows {
  Boundary,
  More,
}

impl From<bool> for Follows {
  fn from(continues: bool) -> Self {
    match bool::then_some(continues, ()) {
      Some(()) => Self::More,
      None => Self::Boundary,
    }
  }
}

pub(crate) fn what_follows(rest: &[u8]) -> Result<Follows, Never> {
  Ok(match rest {
    [] => Follows::Boundary,
    [next, ..] => match class(*next) {
      Ok(Class::WhiteSpace | Class::Delimiter) => Follows::Boundary,
      Ok(Class::Regular) => Follows::More,
    },
  })
}

pub(crate) fn split_while(
  bytes: &[u8],
  classify: impl Fn(u8) -> Follows,
) -> Result<(&[u8], &[u8]), Never> {
  let parts = bytes
    .iter()
    .position(|byte| classify(*byte) == Follows::Boundary)
    .and_then(|end| bytes.split_at_checked(end));

  Ok(match parts {
    Some(split) => split,
    None => (bytes, &[]),
  })
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Walk<'a> {
  pub(crate) from: &'a [u8],
  pub(crate) rest: &'a [u8],
}

impl<'a> Walk<'a> {
  pub(crate) fn walked(self) -> Result<&'a [u8], Never> {
    let before = self
      .from
      .split_at_checked(self.from.len().saturating_sub(self.rest.len()));

    Ok(match before {
      Some((walked, _rest)) => walked,
      None => self.from,
    })
  }
}
