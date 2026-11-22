//! Capture and profile field limits applied while decoding.

use std::fmt;
use std::marker::PhantomData;

use console_core_number_conversion::fixed_index;
use serde::de::{Error, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub(crate) struct Text<const MAX_BYTES: u32>(pub(crate) String);

impl<'de, const MAX_BYTES: u32> Deserialize<'de> for Text<MAX_BYTES> {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    decoder.deserialize_str(TextVisitor::<MAX_BYTES>)
  }
}

struct TextVisitor<const MAX_BYTES: u32>;

impl<'de, const MAX_BYTES: u32> Visitor<'de> for TextVisitor<MAX_BYTES> {
  type Value = Text<MAX_BYTES>;

  fn expecting(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "text of at most {MAX_BYTES} bytes")
  }

  fn visit_str<E: Error>(self, text: &str) -> Result<Self::Value, E> {
    let Ok(bytes) = fixed_index(text.len());

    match bytes.cmp(&u64::from(MAX_BYTES)) {
      std::cmp::Ordering::Greater => {
        Err(E::custom(format_args!("text exceeds {MAX_BYTES} bytes")))
      }
      std::cmp::Ordering::Equal | std::cmp::Ordering::Less => Ok(Text(text.to_owned())),
    }
  }
}

pub(crate) struct List<T, const MAX: u32>(pub(crate) Vec<T>);

impl<T, const MAX: u32> Default for List<T, MAX> {
  fn default() -> Self {
    Self(Vec::new())
  }
}

impl<'de, T: Deserialize<'de>, const MAX: u32> Deserialize<'de> for List<T, MAX> {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    decoder.deserialize_seq(ListVisitor::<T, MAX>(PhantomData))
  }
}

struct ListVisitor<T, const MAX: u32>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const MAX: u32> Visitor<'de> for ListVisitor<T, MAX> {
  type Value = List<T, MAX>;

  fn expecting(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "a sequence of at most {MAX} entries")
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
    let mut entries = Vec::new();

    for _ in 0..MAX {
      let next = sequence.next_element()?;

      match next {
        Some(entry) => entries.push(entry),
        None => return Ok(List(entries)),
      }
    }

    let extra = sequence.next_element::<IgnoredAny>()?;

    match extra {
      Some(_) => Err(A::Error::custom(format_args!(
        "sequence exceeds {MAX} entries"
      ))),
      None => Ok(List(entries)),
    }
  }
}
