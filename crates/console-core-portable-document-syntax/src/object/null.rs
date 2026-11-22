//! The null object.

use crate::ObjectReadMode;
use crate::object::Object;
use crate::object::macros::object;
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, Skippable};

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct Null;

object!(Null, Null);

impl Skippable for Null {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    let Ok(forward_tag) = r.forward_tag(b"null");

    forward_tag
  }
}

impl Readable<'_> for Null {
  fn read(r: &mut Reader<'_>, ctx: &ReaderContext<'_>) -> Option<Self> {
    let Ok(mode) = ctx.read_mode();
    Self::skip(r, mode)?;

    Some(Self)
  }
}

#[cfg(test)]
mod tests {
  use crate::object::Null;
  use crate::reader::{Readable, Reader, ReaderExt};

  fn read<'a, T: Readable<'a>>(data: &'a [u8]) -> Result<Option<T>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);

    Ok(reader.read_without_context::<T>())
  }

  #[test]
  fn null() {
    assert_eq!(read::<Null>(b"null"), Ok(Some(Null)));
  }

  #[test]
  fn null_trailing() {
    assert_eq!(read::<Null>(b"nullabs"), Ok(Some(Null)));
  }
}
