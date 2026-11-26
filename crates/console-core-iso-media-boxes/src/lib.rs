//! Base media box framing for MPEG-4 and QuickTime containers.
//!
//! A box is a size, a four-character code and a payload, and the size counts the header as
//! well as what follows it. The header is eight bytes, eight more where the size is too large
//! for four bytes and follows the code as sixty-four bits, and sixteen more for the user type
//! a `uuid` box carries. A size of zero is a box that runs to the end of whatever holds it, so
//! a header on its own answers `None` and the walk over a payload's children resolves it
//! against what is left. `BoxHeader::required_len` reads the first eight bytes and says how
//! long the whole header is, so a reader of a stream asks for exactly the bytes still missing
//! before it parses.
//!
//! The walk over children stops at the first malformed one and says so once: a size that
//! cannot hold its own header, or runs past its container, leaves nothing after it worth
//! trusting. Zeros after the last child, too few to be a header, are the padding QuickTime
//! writes, and they end the walk quietly rather than as a fault.
//!
//! Adapted from the clean-room base media reader carried in this workspace.
//! The original MIT and Apache-2.0 licenses are retained beside this crate.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod codes;

pub use codes::FourCharacterCode;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const USER_TYPE: FourCharacterCode = FourCharacterCode(*b"uuid");

const USER_TYPE_HEADER_LEN: u64 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxError {
  Truncated {
    required: u64,
  },
  InvalidSize {
    kind: FourCharacterCode,
    size: u64,
    header_len: u64,
  },
  ExceedsContainer {
    kind: FourCharacterCode,
    size: u64,
    available: u64,
  },
}

impl core::fmt::Display for BoxError {
  fn fmt(&self, to: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Truncated { required } => write!(to, "box header requires {required} bytes"),
      Self::InvalidSize { kind, size, .. } => {
        write!(to, "box '{kind}' size {size} smaller than its header")
      }
      Self::ExceedsContainer {
        kind,
        size,
        available,
      } => write!(
        to,
        "box '{kind}' size {size} exceeds its container ({available} bytes left)"
      ),
    }
  }
}

impl core::error::Error for BoxError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxHeader {
  pub kind: FourCharacterCode,
  pub header_len: u64,
  pub size: Option<u64>,
  pub user_type: Option<[u8; 16]>,
}

impl BoxHeader {
  pub const BASE_HEADER_LEN: u64 = 8;
  pub const EXTENDED_HEADER_LEN: u64 = 16;
  pub const MAX_HEADER_LEN: u64 = 32;
  pub const BUFFER: [u8; 32] = [0; 32];

  pub fn required_len(data: &[u8]) -> Result<u64, BoxError> {
    let opening = opened(data)?;

    Ok(
      match (opening.size, UserTypeField::from(opening.kind == USER_TYPE)) {
        (SizeField::Large, UserTypeField::Present) => Self::MAX_HEADER_LEN,
        (SizeField::Large, UserTypeField::Absent) => Self::EXTENDED_HEADER_LEN,
        (SizeField::ToTheEnd | SizeField::Compact(_), UserTypeField::Present) => {
          USER_TYPE_HEADER_LEN
        }
        (SizeField::ToTheEnd | SizeField::Compact(_), UserTypeField::Absent) => {
          Self::BASE_HEADER_LEN
        }
      },
    )
  }

  pub fn parse(data: &[u8]) -> Result<Self, BoxError> {
    let required = Self::required_len(data)?;
    let truncated = BoxError::Truncated { required };
    let Ok(width) = index(required);
    let header = data.get(..width).ok_or(truncated)?;
    let opening = opened(header)?;
    let (size, after_size) = match opening.size {
      SizeField::ToTheEnd => (None, opening.after),
      SizeField::Compact(written) => (Some(u64::from(written)), opening.after),
      SizeField::Large => {
        let (large, after_large) = opening.after.split_first_chunk::<8>().ok_or(truncated)?;

        (Some(u64::from_be_bytes(*large)), after_large)
      }
    };
    let user_type = match UserTypeField::from(opening.kind == USER_TYPE) {
      UserTypeField::Present => {
        let (user_type, _rest_of_header) =
          after_size.split_first_chunk::<16>().ok_or(truncated)?;

        Some(*user_type)
      }
      UserTypeField::Absent => None,
    };

    match size.filter(|total| *total < required) {
      Some(smaller) => Err(BoxError::InvalidSize {
        kind: opening.kind,
        size: smaller,
        header_len: required,
      }),
      None => Ok(Self {
        kind: opening.kind,
        header_len: required,
        size,
        user_type,
      }),
    }
  }

  pub fn payload_len(&self) -> Result<Option<u64>, Never> {
    Ok(self.size.and_then(|size| size.checked_sub(self.header_len)))
  }
}

#[derive(Clone, Copy)]
enum SizeField {
  ToTheEnd,
  Large,
  Compact(u32),
}

enum UserTypeField {
  Present,
  Absent,
}

impl From<bool> for UserTypeField {
  fn from(present: bool) -> Self {
    match bool::then_some(present, ()) {
      Some(()) => Self::Present,
      None => Self::Absent,
    }
  }
}

struct Opening<'a> {
  size: SizeField,
  kind: FourCharacterCode,
  after: &'a [u8],
}

fn opened(data: &[u8]) -> Result<Opening<'_>, BoxError> {
  let short = BoxError::Truncated {
    required: BoxHeader::BASE_HEADER_LEN,
  };
  let (size, after_size) = data.split_first_chunk::<4>().ok_or(short)?;
  let (kind, after) = after_size.split_first_chunk::<4>().ok_or(short)?;
  let field = match u32::from_be_bytes(*size) {
    0 => SizeField::ToTheEnd,
    1 => SizeField::Large,
    written => SizeField::Compact(written),
  };

  Ok(Opening {
    size: field,
    kind: FourCharacterCode(*kind),
    after,
  })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullBoxHeader {
  pub version: u8,
  pub flags: u32,
}

impl FullBoxHeader {
  pub const ENCODED_LEN: u64 = 4;

  pub fn from_bytes([version, high, middle, low]: [u8; 4]) -> Result<Self, Never> {
    let Ok(flags) = console_core_bytes::endian::u24_be([high, middle, low]);

    Ok(Self { version, flags })
  }
}

#[derive(Clone, Copy, Debug)]
pub struct BoxView<'a> {
  pub kind: FourCharacterCode,
  pub payload: &'a [u8],
  pub user_type: Option<[u8; 16]>,
}

pub struct Children<'a> {
  rest: &'a [u8],
}

impl<'a> Children<'a> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    Ok(Self { rest: data })
  }
}

impl<'a> Iterator for Children<'a> {
  type Item = Result<BoxView<'a>, BoxError>;

  fn next(&mut self) -> Option<Self::Item> {
    let rest = core::mem::take(&mut self.rest);
    let Ok(step) = stepped(rest);

    match step {
      Step::End => None,
      Step::Damaged(fault) => Some(Err(fault)),
      Step::Child { child, after } => {
        self.rest = after;

        Some(Ok(child))
      }
    }
  }
}

enum Step<'a> {
  End,
  Damaged(BoxError),
  Child { child: BoxView<'a>, after: &'a [u8] },
}

enum TrailingData {
  Padding,
  Malformed,
}

impl From<bool> for TrailingData {
  fn from(padding: bool) -> Self {
    match bool::then_some(padding, ()) {
      Some(()) => Self::Padding,
      None => Self::Malformed,
    }
  }
}

fn stepped(rest: &[u8]) -> Result<Step<'_>, Never> {
  let header = match BoxHeader::parse(rest) {
    Ok(header) => header,
    Err(fault) => return padding_or(rest, fault),
  };
  let Ok(available) = fitted::<_, u64>(rest.len());
  let size = match header.size {
    Some(declared) => declared,
    None => available,
  };
  let exceeds = BoxError::ExceedsContainer {
    kind: header.kind,
    size,
    available,
  };
  let Ok(end) = index(size);
  let Ok(start) = index(header.header_len);

  Ok(match (rest.get(start..end), rest.get(end..)) {
    (Some(payload), Some(after)) => Step::Child {
      child: BoxView {
        kind: header.kind,
        payload,
        user_type: header.user_type,
      },
      after,
    },
    (None, Some(_) | None) | (Some(_), None) => Step::Damaged(exceeds),
  })
}

fn padding_or(rest: &[u8], fault: BoxError) -> Result<Step<'_>, Never> {
  let Ok(length) = fitted::<_, u64>(rest.len());

  let trailing =
    TrailingData::from(length < BoxHeader::BASE_HEADER_LEN && rest.iter().all(|byte| *byte == 0));

  Ok(match trailing {
    TrailingData::Padding => Step::End,
    TrailingData::Malformed => Step::Damaged(fault),
  })
}
