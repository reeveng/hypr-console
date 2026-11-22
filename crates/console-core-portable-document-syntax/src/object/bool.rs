//! Booleans.

use crate::ObjectReadMode;
use crate::object::Object;
use crate::object::macros::object;
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use core::fmt;

#[doc = "A boolean literal in a PDF object."]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum PdfBoolean {
  True,
  #[default]
  False,
}

impl fmt::Debug for PdfBoolean {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(match self {
      Self::True => "true",
      Self::False => "false",
    })
  }
}

impl From<PdfBoolean> for bool {
  fn from(value: PdfBoolean) -> Self {
    matches!(value, PdfBoolean::True)
  }
}

impl Skippable for PdfBoolean {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    let Ok(peek_byte) = r.peek_byte();

    let checked_value = peek_byte?;

    match checked_value {
      b't' => {
        let Ok(forward_tag) = r.forward_tag(b"true");

        forward_tag
      }
      b'f' => {
        let Ok(forward_tag) = r.forward_tag(b"false");

        forward_tag
      }
      _ => None,
    }
  }
}

impl Readable<'_> for PdfBoolean {
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    let checked_value = r.skip::<Self>(ObjectReadMode::ContentStream)?;

    match checked_value {
      b"true" => Some(PdfBoolean::True),
      b"false" => Some(PdfBoolean::False),
      _ => None,
    }
  }
}

object!(PdfBoolean, Boolean);

#[cfg(test)]
mod tests {
  use super::PdfBoolean;
  use crate::reader::{Readable, Reader, ReaderExt};

  fn read<'a, T: Readable<'a>>(data: &'a [u8]) -> Result<Option<T>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);

    Ok(reader.read_without_context::<T>())
  }

  #[test]
  fn bool_true() {
    assert_eq!(read::<PdfBoolean>(b"true"), Ok(Some(PdfBoolean::True)));
  }

  #[test]
  fn bool_false() {
    assert_eq!(read::<PdfBoolean>(b"false"), Ok(Some(PdfBoolean::False)));
  }

  #[test]
  fn bool_trailing() {
    assert_eq!(read::<PdfBoolean>(b"trueabdf"), Ok(Some(PdfBoolean::True)));
  }
}
