//! JPEG: a start-of-image marker, then segments until the one that starts
//! the frame, which says the height and then the width, two bytes each and
//! highest first.
//!
//! Every segment begins with `FF` and a marker. Most carry their length next,
//! counting the two bytes it is written in, and are stepped over whole, which
//! is also how a thumbnail inside an Exif segment is never mistaken for the
//! picture. The restart markers, a temporary one and a second start of image
//! carry no length. A frame starts at any SOFn marker: baseline, extended,
//! progressive and lossless, Huffman or arithmetic, which is every marker from
//! `C0` to `CF` but `C4`, `C8` and `CC`, which are tables and a reserved one.
//! An end of image or the start of a scan before any frame is a picture with
//! no size in it.
//!
//! The two parsers this replaces read the same segments differently.
//! Applications knew only the first four frame markers, so a progressive
//! arithmetic picture had no size; the fork's whole list is kept. The fork
//! called any byte between segments that was not `FF` a corrupt picture, and
//! read a length after a marker that has none; Applications stepped over such
//! a byte and looked again, as libjpeg does with what it calls extraneous
//! bytes before a marker, and that is kept, with any run of `FF` before a
//! marker read as the fill bytes the standard allows.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_size::Size;

use crate::{Signature, SizeError, signature, two};

const SIGNATURE: &[u8] = b"\xFF\xD8\xFF";

const MARK: u8 = 0xFF;

enum Segment<'a> {
  Frame(Size<u32>),
  Next(&'a [u8]),
}

pub(crate) fn begins(head: &[u8]) -> Result<Signature, Never> {
  signature(head, SIGNATURE)
}

pub(crate) fn size(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let Ok(bound) = fitted::<_, u64>(head.len());

  let mut rest = match head.get(2..) {
    Some(segments) => segments,
    None => return Err(SizeError::Truncated),
  };

  for _segment in 0..=bound {
    match rest {
      [] | [MARK] => return Err(SizeError::Truncated),
      [MARK, MARK, fill_on @ ..] => {
        rest = match rest.split_first() {
          Some((_fill, marked)) => marked,
          None => fill_on,
        };
      }
      [MARK, marker, after @ ..] => {
        let found = segment(*marker, after)?;

        match found {
          Segment::Frame(size) => return Ok(size),
          Segment::Next(next) => rest = next,
        }
      }
      [_extraneous, after @ ..] => rest = after,
    }
  }

  Err(SizeError::Truncated)
}

fn segment(marker: u8, after: &[u8]) -> Result<Segment<'_>, SizeError> {
  match marker {
    0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => frame(after),
    0x00 | 0x01 | 0xD0..=0xD8 => Ok(Segment::Next(after)),
    0xD9 | 0xDA => Err(SizeError::Corrupted),
    _sized => skipped(after),
  }
}

fn frame(after: &[u8]) -> Result<Segment<'_>, SizeError> {
  let height = two(after.get(3..))?;
  let width = two(after.get(5..))?;

  Ok(Segment::Frame(Size {
    width: u32::from(u16::from_be_bytes(width)),
    height: u32::from(u16::from_be_bytes(height)),
  }))
}

fn skipped(after: &[u8]) -> Result<Segment<'_>, SizeError> {
  let said = two(Some(after))?;
  let length = u16::from_be_bytes(said);

  match length {
    0 | 1 => Err(SizeError::Corrupted),
    _counted => {
      let Ok(skip) = index(length);

      match after.get(skip..) {
        Some(next) => Ok(Segment::Next(next)),
        None => Err(SizeError::Truncated),
      }
    }
  }
}
