//! An array, kept as the bytes between its brackets until somebody scans
//! what is inside.
//!
//! The end of an array is the bracket that balances its opening one, and a
//! bracket inside a string or a comment is not one, so the walk to it steps
//! over strings and comments whole and counts only the brackets outside them.
//! What it finds is a scanner of its own, so an array inside an array is read
//! the way its outer one was, when it is wanted.

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::bytes::Walk;
use crate::object::blank_skipped;
use crate::string::{self, Encoding};
use crate::{ScanError, Scanner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Array<'a> {
  body: &'a [u8],
}

impl<'a> Array<'a> {
  pub fn objects(&self) -> Result<Scanner<'a>, Never> {
    Scanner::new(self.body)
  }
}

pub(crate) fn delimited(after_open: &[u8]) -> Result<(Array<'_>, &[u8]), ScanError> {
  let Ok(bound) = fitted::<_, u64>(after_open.len());
  let mut depth = 1u32;
  let mut rest = after_open;

  for _step in 0..=bound {
    match rest {
      [] => return Err(ScanError::SyntaxError),
      [b'[', after @ ..] => {
        depth = depth.saturating_add(1);
        rest = after;
      }
      [b']', after @ ..] => {
        depth = depth.saturating_sub(1);

        match depth {
          0 => {
            let Ok(body) = (Walk {
              from: after_open,
              rest,
            })
            .walked();

            return Ok((Array { body }, after));
          }
          _open => rest = after,
        }
      }
      [_first, ..] => {
        let after = stepped(rest)?;

        rest = after;
      }
    }
  }

  Err(ScanError::SyntaxError)
}

fn stepped(rest: &[u8]) -> Result<&[u8], ScanError> {
  match rest {
    [] => Err(ScanError::SyntaxError),
    [b'(', after @ ..] => string::skipped(after, Encoding::Literal),
    [b'<', b'~', after @ ..] => string::skipped(after, Encoding::Ascii85),
    [b'<', b'<', after @ ..] => Ok(after),
    [b'<', after @ ..] => string::skipped(after, Encoding::Hexadecimal),
    [b'%', ..] => {
      let Ok(after) = blank_skipped(rest);

      Ok(after)
    }
    [_byte, after @ ..] => Ok(after),
  }
}
