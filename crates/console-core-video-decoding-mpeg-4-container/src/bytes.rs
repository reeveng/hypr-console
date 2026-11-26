//! The bytes a box's payload is read in: a cursor over them, the walk over the
//! boxes a payload holds, and the first of those with a given name.
//!
//! Every number in a base media file is big-endian, and every read here is
//! checked against what is left, so a table claiming more than its box holds is
//! a truncated box rather than a read past its end. The cursor keeps what is
//! left of its payload rather than a position in it, so what has been read is
//! gone from it and there is no number to keep in step with the bytes. A box's
//! framing -- its size, its name and the longer forms of both -- is
//! `console-core-iso-media-boxes`, which the audio reader reads it through too.

use console_core_bytes::Cursor as ByteCursor;
use console_core_iso_media_boxes::{BoxView, Children, FullBoxHeader};
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::error::Error;

pub use console_core_iso_media_boxes::FourCharacterCode as FourCc;

#[cfg_attr(
  dylint_lib = "explicit089_no_lossy_text",
  expect(
    explicit089_no_lossy_text,
    reason = "these labels and metadata values are presented to a reader, while codec selection and metadata key matching use their own validated fields"
  )
)]
pub(crate) fn display_text(bytes: &[u8]) -> Result<String, Never> {
  Ok(String::from_utf8_lossy(bytes).into_owned())
}

#[derive(Clone)]
pub(crate) struct Cursor<'a> {
  bytes: ByteCursor<'a>,
  part: &'static str,
}

impl<'a> Cursor<'a> {
  pub(crate) fn new(data: &'a [u8], part: &'static str) -> Result<Self, Never> {
    let Ok(bytes) = ByteCursor::new(data);

    Ok(Cursor { bytes, part })
  }

  pub(crate) fn remaining(&self) -> Result<u64, Never> {
    let Ok(rest) = self.bytes.rest();

    fitted::<_, u64>(rest.len())
  }

  pub(crate) fn rest(&self) -> Result<&'a [u8], Never> {
    self.bytes.rest()
  }

  pub(crate) fn bytes(&mut self, count: u64) -> Result<&'a [u8], Error> {
    self
      .bytes
      .take(count)
      .map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn skip(&mut self, count: u64) -> Result<(), Error> {
    let _skipped = self.bytes(count)?;

    Ok(())
  }

  pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
    self.bytes.array().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn u8(&mut self) -> Result<u8, Error> {
    self.bytes.byte().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn u16(&mut self) -> Result<u16, Error> {
    self.bytes.u16_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn i16(&mut self) -> Result<i16, Error> {
    self.bytes.i16_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn u24(&mut self) -> Result<u32, Error> {
    self.bytes.u24_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn u32(&mut self) -> Result<u32, Error> {
    self.bytes.u32_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn i32(&mut self) -> Result<i32, Error> {
    self.bytes.i32_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn u64(&mut self) -> Result<u64, Error> {
    self.bytes.u64_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn i64(&mut self) -> Result<i64, Error> {
    self.bytes.i64_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn f64(&mut self) -> Result<f64, Error> {
    self.bytes.f64_be().map_err(|_| Error::Truncated(self.part))
  }

  pub(crate) fn fourcc(&mut self) -> Result<FourCc, Error> {
    let code = self.array()?;

    Ok(FourCc(code))
  }

  pub(crate) fn full_header(&mut self) -> Result<FullBoxHeader, Error> {
    let bytes = self.array()?;
    let Ok(header) = FullBoxHeader::from_bytes(bytes);

    Ok(header)
  }

  pub(crate) fn count(&mut self, entry_size: u64) -> Result<u32, Error> {
    let count = self.u32()?;

    self.check_count(count, entry_size)?;

    Ok(count)
  }

  pub(crate) fn check_count(&self, count: u32, entry_size: u64) -> Result<(), Error> {
    let Ok(remaining) = self.remaining();

    match u64::from(count).checked_mul(entry_size) {
      Some(needed) => match ::core::primitive::bool::then_some(needed <= remaining, ()) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => Err(Error::Truncated(self.part)),
      },
      None => Err(Error::Truncated(self.part)),
    }
  }
}

pub(crate) fn boxes(
  data: &[u8],
) -> Result<impl Iterator<Item = Result<BoxView<'_>, Error>>, Never> {
  let Ok(children) = Children::new(data);

  Ok(children.map(|child| child.map_err(Error::from)))
}

pub(crate) fn whole_boxes(data: &[u8]) -> Result<impl Iterator<Item = BoxView<'_>>, Never> {
  let Ok(children) = Children::new(data);

  Ok(children.map_while(|child| match child {
    Ok(view) => Some(view),
    Err(_damaged) => None,
  }))
}

pub(crate) fn find<'a>(data: &'a [u8], kind: &[u8; 4]) -> Result<Option<&'a [u8]>, Never> {
  let Ok(mut children) = whole_boxes(data);

  Ok(
    children
      .find(|child| child.kind == *kind)
      .map(|child| child.payload),
  )
}

#[cfg(test)]
pub(crate) mod building {
  //! Boxes written by hand for the tests, which is all the writing this crate does.

  use console_core_never::Never;
  use console_core_number_conversion::fitted;

  pub(crate) fn claiming(kind: &[u8; 4], body: &[u8], size: u32) -> Result<Vec<u8>, Never> {
    Ok([size.to_be_bytes().as_slice(), kind.as_slice(), body].concat())
  }

  pub(crate) fn boxed(kind: &[u8; 4], body: &[u8]) -> Result<Vec<u8>, Never> {
    let Ok(length) = fitted::<_, u32>(body.len());

    claiming(kind, body, length.saturating_add(8))
  }

  pub(crate) fn full(kind: &[u8; 4], version: u8, body: &[u8]) -> Result<Vec<u8>, Never> {
    boxed(kind, &[[version, 0, 0, 0].as_slice(), body].concat())
  }

  pub(crate) fn words(values: &[u32]) -> Result<Vec<u8>, Never> {
    Ok(
      values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect(),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::building::boxed;
  use super::*;

  use console_core_iso_media_boxes::{BoxError, BoxHeader};

  type Outcome = Result<(), Box<dyn std::error::Error>>;

  #[test]
  fn header_forms() -> Outcome {
    let free = BoxHeader::parse(&[0, 0, 0, 16, b'f', b'r', b'e', b'e'])?;

    assert_eq!(
      (free.kind, free.header_len, free.size),
      (FourCc(*b"free"), 8, Some(16))
    );

    let to_the_end = BoxHeader::parse(&[0, 0, 0, 0, b'm', b'd', b'a', b't'])?;

    assert_eq!(to_the_end.size, None);

    let mut large = vec![0, 0, 0, 1, b'm', b'd', b'a', b't'];

    large.extend_from_slice(&100u64.to_be_bytes());

    let large_header = BoxHeader::parse(&large)?;

    assert_eq!(
      (large_header.header_len, large_header.size),
      (16, Some(100))
    );

    let mut user = vec![0, 0, 0, 30, b'u', b'u', b'i', b'd'];

    user.extend_from_slice(&[7u8; 16]);

    let user_header = BoxHeader::parse(&user)?;

    assert_eq!(
      (user_header.header_len, user_header.user_type),
      (24, Some([7u8; 16]))
    );
    assert!(matches!(
      BoxHeader::parse(&[0, 0, 0, 4, b'f', b'r', b'e', b'e']),
      Err(BoxError::InvalidSize { .. })
    ));
    assert!(matches!(
      BoxHeader::parse(&[0, 0, 0]),
      Err(BoxError::Truncated { .. })
    ));

    Ok(())
  }

  #[test]
  fn iterate_children() -> Outcome {
    let Ok(first) = boxed(b"aaaa", &[1, 2]);
    let Ok(inner) = boxed(b"cccc", &[]);
    let Ok(second) = boxed(b"bbbb", &inner);
    let built = [first, second, vec![0, 0, 0, 0]].concat();
    let Ok(walked) = boxes(&built);
    let children = walked.collect::<Result<Vec<_>, Error>>()?;
    let payloads: Vec<&[u8]> = children.iter().map(|child| child.payload).collect();

    assert_eq!(payloads, [[1u8, 2].as_slice(), inner.as_slice()]);

    let Ok(found) = find(&built, b"bbbb");

    assert_eq!(found.map(<[u8]>::len), Some(8));

    let Ok(mut damaged) = boxes(&[0u8, 0, 0, 99, b'x', b'x', b'x', b'x', 0]);

    assert!(matches!(
      damaged.next(),
      Some(Err(Error::Framing(BoxError::ExceedsContainer { .. })))
    ));

    Ok(())
  }

  #[test]
  fn cursor_bounds() {
    let Ok(mut short) = Cursor::new(&[0, 0, 0, 5, 1], "t");

    assert!(matches!(short.count(1), Err(Error::Truncated("t"))));

    let Ok(mut huge) = Cursor::new(&[0xFF, 0xFF, 0xFF, 0xFF], "t");

    assert!(matches!(huge.count(u64::MAX), Err(Error::Truncated("t"))));
  }
}
