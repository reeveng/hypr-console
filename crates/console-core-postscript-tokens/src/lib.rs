//! The objects a PostScript program is written in, read off it one at a time.
//!
//! A CMap is a PostScript program, and reading one wants numbers, names,
//! strings and arrays and nothing else: no dictionary is built and no
//! procedure is run. A [`Scanner`] hands back each object as it comes to it,
//! borrowed from the program, and a string is decoded only when somebody asks
//! for its bytes. `<<` and `>>` are stepped over, so what a dictionary holds
//! comes out as the objects inside it, and `{` is said to be a procedure
//! rather than read.
//!
//! The crate began as hayro-postscript, by Laurenz Stampfl under MIT or
//! Apache-2.0, carried in `forks/` for the CMap reader, and it came into the
//! tree under the rules. A token is read off the front of what is left of the
//! program rather than at an offset into it, so nothing is indexed and no
//! position can pass the end. The fork read the object after a `<<` by
//! calling itself, which a program of nested `<<` could run the stack out
//! with; every one is now stepped over in a walk that stops at the program's
//! end. A name says whether it is literal or executable rather than answering
//! a bool, a name that is not UTF-8 says why, and an object of another kind
//! than the one asked for is PostScript's typecheck rather than a syntax
//! error. What nobody read was left behind: a number as either float, an
//! array asked for by kind, and a limit no call reached.
//!
//! Every answer the fork gave about what a program says is kept, including
//! the ones PostScript itself would give otherwise: a token that starts the
//! way a number does and is not one is a fault rather than an executable name,
//! an integer past `i32` is a fault rather than a real, and an octal escape
//! past a byte says nothing.

mod array;
mod bytes;
pub mod decoding;
mod name;
mod number;
mod object;
mod string;

pub use crate::array::Array;
pub use crate::name::{Name, NameKind};
pub use crate::number::Number;
pub use crate::object::Object;
pub use crate::string::String;

use console_core_never::Never;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
  Ended,
  SyntaxError,
  TypeCheck,
  Unsupported,
}

impl fmt::Display for ScanError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ScanError::Ended => write!(to, "the program ended where an object was wanted"),
      ScanError::SyntaxError => write!(to, "these bytes are no PostScript object"),
      ScanError::TypeCheck => write!(to, "the object is of another kind than the one asked for"),
      ScanError::Unsupported => write!(to, "a procedure, which this scanner does not read"),
    }
  }
}

impl std::error::Error for ScanError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remaining {
  Objects,
  Nothing,
}

#[derive(Debug, Clone)]
pub struct Scanner<'a> {
  rest: &'a [u8],
}

impl<'a> Scanner<'a> {
  pub fn new(program: &'a [u8]) -> Result<Scanner<'a>, Never> {
    Ok(Scanner { rest: program })
  }

  pub fn remaining(&mut self) -> Result<Remaining, Never> {
    let Ok(rest) = object::blank_skipped(self.rest);

    self.rest = rest;

    Ok(match rest {
      [] => Remaining::Nothing,
      [_first, ..] => Remaining::Objects,
    })
  }

  pub fn parse_object(&mut self) -> Result<Object<'a>, ScanError> {
    let (object, rest) = object::read(self.rest)?;

    self.rest = rest;

    Ok(object)
  }

  pub fn parse_number(&mut self) -> Result<Number, ScanError> {
    let object = self.parse_object()?;

    match object {
      Object::Number(number) => Ok(number),
      Object::Name(_) | Object::String(_) | Object::Array(_) => Err(ScanError::TypeCheck),
    }
  }

  pub fn parse_name(&mut self) -> Result<Name<'a>, ScanError> {
    let object = self.parse_object()?;

    match object {
      Object::Name(name) => Ok(name),
      Object::Number(_) | Object::String(_) | Object::Array(_) => Err(ScanError::TypeCheck),
    }
  }

  pub fn parse_string(&mut self) -> Result<String<'a>, ScanError> {
    let object = self.parse_object()?;

    match object {
      Object::String(string) => Ok(string),
      Object::Number(_) | Object::Name(_) | Object::Array(_) => Err(ScanError::TypeCheck),
    }
  }
}
