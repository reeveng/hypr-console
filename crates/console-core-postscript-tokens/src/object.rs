//! One object off the front of a program, after the white space and comments
//! before it.
//!
//! An object is told by its first byte: a parenthesis, `<` or `<~` opens a
//! string, `/` a literal name, `[` an array, a digit, sign or point a number,
//! and anything else that is part of a token an executable name. `<<` and
//! `>>` are stepped over wherever an object is looked for, as many as there
//! are, because a dictionary is not built here and what it holds is read as
//! the objects it is.

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::ScanError;
use crate::array::{self, Array};
use crate::bytes::{Class, Follows, Line, class, line, split_while};
use crate::name::{self, Name};
use crate::number::{self, Number};
use crate::string::{self, Encoding, String};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Object<'a> {
  Number(Number),
  Name(Name<'a>),
  String(String<'a>),
  Array(Array<'a>),
}

pub(crate) fn blank_skipped(program: &[u8]) -> Result<&[u8], Never> {
  let Ok(bound) = fitted::<_, u64>(program.len());
  let mut rest = program;

  for _blank in 0..=bound {
    match rest {
      [] => return Ok(rest),
      [b'%', after @ ..] => {
        let Ok((_comment, line_on)) = split_while(after, |byte| {
          Follows::from(line(byte) == Ok(Line::Continues))
        });

        rest = line_on;
      }
      [first, after @ ..] => match class(*first) {
        Ok(Class::WhiteSpace) => rest = after,
        Ok(Class::Delimiter | Class::Regular) => return Ok(rest),
      },
    }
  }

  Ok(rest)
}

pub(crate) fn read(program: &[u8]) -> Result<(Object<'_>, &[u8]), ScanError> {
  let Ok(bound) = fitted::<_, u64>(program.len());
  let mut rest = program;

  for _marker in 0..=bound {
    let Ok(start) = blank_skipped(rest);

    match start {
      [] => return Err(ScanError::Ended),
      [b'<', b'<', after @ ..] | [b'>', b'>', after @ ..] => rest = after,
      [_first, ..] => return token(start),
    }
  }

  Err(ScanError::Ended)
}

fn token(start: &[u8]) -> Result<(Object<'_>, &[u8]), ScanError> {
  match start {
    [b'(', after @ ..] => string::delimited(after, Encoding::Literal)
      .map(|(found, rest)| (Object::String(found), rest)),
    [b'<', b'~', after @ ..] => string::delimited(after, Encoding::Ascii85)
      .map(|(found, rest)| (Object::String(found), rest)),
    [b'<', after @ ..] => string::delimited(after, Encoding::Hexadecimal)
      .map(|(found, rest)| (Object::String(found), rest)),
    [b'/', after @ ..] => {
      let Ok((found, rest)) = name::literal(after);

      Ok((Object::Name(found), rest))
    }
    [b'[', after @ ..] => {
      array::delimited(after).map(|(found, rest)| (Object::Array(found), rest))
    }
    [b'{', ..] => Err(ScanError::Unsupported),
    [b'.' | b'+' | b'-' | b'0'..=b'9', ..] => {
      number::read(start).map(|(found, rest)| (Object::Number(found), rest))
    }
    _executable => name::executable(start).map(|(found, rest)| (Object::Name(found), rest)),
  }
}
