//! A tag: the four bytes that name a table, a script, a language, a feature or a variation axis.
//!
//! A tag read from a font keeps whatever four bytes the font wrote, because a reader has to be
//! able to name a table it does not understand. One made from text is held to what the
//! specification allows -- one to four printable ASCII characters, padded with spaces, with
//! nothing printable after the first space -- and `validate` asks the same of one that was read.
//! It prints as its characters, with any byte that is not printable written as its hexadecimal
//! value in braces.

use console_core_never::Never;
use console_core_number_conversion::fitted;
use core::{
  borrow::Borrow,
  fmt::{Debug, Display, Formatter},
  str::FromStr,
};

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "text parsing validates the printable tag syntax; the binary representation deliberately preserves every four-byte identifier, including the blank default"
  )
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Tag([u8; 4]);

const BLANK_TAG: [u8; 4] = [b' '; 4];

enum TagPart {
  Name,
  Padding,
}

impl Tag {
  pub const fn new(src: &[u8; 4]) -> Result<Tag, Never> {
    Ok(Tag(*src))
  }

  pub fn new_checked(src: &[u8]) -> Result<Self, InvalidTag> {
    let Ok(length) = fitted::<_, u64>(src.len());

    match length {
      1..=4 => {}
      0 | 5.. => return Err(InvalidTag::InvalidLength(length)),
    }

    let mut raw = BLANK_TAG;
    let mut part = TagPart::Name;

    for (pos, (slot, byte)) in (0u32..).zip(raw.iter_mut().zip(src.iter().copied())) {
      match (byte, pos, &part) {
        (0x20, 0, _) | (0..=0x1f | 0x7f.., _, _) | (0x21..=0x7e, _, TagPart::Padding) => {
          return Err(InvalidTag::InvalidByte { pos, byte });
        }
        (0x20, _, _) => part = TagPart::Padding,
        (0x21..=0x7e, _, TagPart::Name) => {}
      }

      *slot = byte;
    }

    Ok(Self(raw))
  }

  pub const fn from_u32(src: u32) -> Result<Self, Never> {
    Self::from_be_bytes(src.to_be_bytes())
  }

  pub const fn from_be_bytes(bytes: [u8; 4]) -> Result<Self, Never> {
    Ok(Self(bytes))
  }

  pub const fn to_be_bytes(self) -> Result<[u8; 4], Never> {
    Ok(self.0)
  }

  pub fn into_bytes(self) -> Result<[u8; 4], Never> {
    Ok(self.0)
  }

  pub fn validate(self) -> Result<(), InvalidTag> {
    match self.0 {
      BLANK_TAG => return Err(InvalidTag::InvalidLength(0)),
      _ => {}
    }

    let mut part = TagPart::Name;

    for (pos, byte) in (0u32..).zip(self.0) {
      match (byte, pos, &part) {
        (0x20, 0, _) | (0..=0x1f | 0x7f.., _, _) => {
          return Err(InvalidTag::InvalidByte { pos, byte });
        }
        (0x20, _, _) => part = TagPart::Padding,
        (0x21..=0x7e, _, TagPart::Padding) => return Err(InvalidTag::ByteAfterSpace { pos }),
        (0x21..=0x7e, _, TagPart::Name) => {}
      }
    }

    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidTag {
  InvalidLength(u64),
  InvalidByte { pos: u32, byte: u8 },
  ByteAfterSpace { pos: u32 },
}

impl FromStr for Tag {
  type Err = InvalidTag;

  fn from_str(src: &str) -> Result<Self, Self::Err> {
    Tag::new_checked(src.as_bytes())
  }
}

impl crate::raw::Scalar for Tag {
  type Raw = [u8; 4];

  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.to_be_bytes()
  }

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    Self::from_be_bytes(raw)
  }
}

impl core::error::Error for InvalidTag {}

impl Borrow<[u8; 4]> for Tag {
  fn borrow(&self) -> &[u8; 4] {
    &self.0
  }
}

impl PartialEq<[u8; 4]> for Tag {
  fn eq(&self, other: &[u8; 4]) -> bool {
    &self.0 == other
  }
}

impl PartialEq<str> for Tag {
  fn eq(&self, other: &str) -> bool {
    self.0 == other.as_bytes()
  }
}

impl PartialEq<&str> for Tag {
  fn eq(&self, other: &&str) -> bool {
    self == *other
  }
}

impl PartialEq<&[u8]> for Tag {
  fn eq(&self, other: &&[u8]) -> bool {
    self.0.as_ref() == *other
  }
}

impl AsRef<[u8]> for Tag {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl Display for Tag {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    for byte in self.0 {
      let written = match byte {
        0x20..=0x7e => write!(f, "{}", char::from(byte)),
        0..=0x1f | 0x7f.. => write!(f, "{{0x{byte:02X}}}"),
      };

      written?;
    }

    Ok(())
  }
}

impl Display for InvalidTag {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    match self {
      InvalidTag::InvalidByte { pos, byte } => {
        write!(f, "Invalid byte 0x{byte:X} at index {pos}")
      }
      InvalidTag::InvalidLength(len) => write!(f, "Invalid length ({len})"),
      InvalidTag::ByteAfterSpace { .. } => write!(f, "Non-space character after first space"),
    }
  }
}

impl Debug for Tag {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "Tag({})", self)
  }
}

impl Default for Tag {
  fn default() -> Self {
    Tag(BLANK_TAG)
  }
}

#[cfg(feature = "serde")]
enum TagEncoding {
  Text,
  Bytes,
}

#[cfg(feature = "serde")]
impl From<bool> for TagEncoding {
  fn from(human_readable: bool) -> Self {
    match human_readable.then_some(()) {
      Some(()) => Self::Text,
      None => Self::Bytes,
    }
  }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Tag {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match TagEncoding::from(serializer.is_human_readable()) {
      TagEncoding::Text => {
        let text = core::str::from_utf8(&self.0).map_err(serde::ser::Error::custom)?;

        text.serialize(serializer)
      }
      TagEncoding::Bytes => self.0.serialize(serializer),
    }
  }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Tag {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct TagStrVisitor;

    impl serde::de::Visitor<'_> for TagStrVisitor {
      type Value = Tag;

      fn expecting(&self, formatter: &mut Formatter) -> core::fmt::Result {
        write!(formatter, "a four-byte ascii string")
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        v.parse().map_err(serde::de::Error::custom)
      }
    }

    match TagEncoding::from(deserializer.is_human_readable()) {
      TagEncoding::Text => deserializer.deserialize_str(TagStrVisitor),
      TagEncoding::Bytes => <[u8; 4]>::deserialize(deserializer).map(Tag),
    }
  }
}
