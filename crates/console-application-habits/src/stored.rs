//! Read the recorder's state under explicit file and field size limits.

use std::cmp::Ordering;
use std::fs::File;
use std::io;
use std::path::Path;

use console_core_number_conversion::index;
use serde::{Deserialize, Deserializer, de};

use crate::Result;

pub(crate) struct Text<const LIMIT: u32>(pub String);

impl<'de, const LIMIT: u32> Deserialize<'de> for Text<LIMIT> {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
    let value = String::deserialize(decoder)?;
    let Ok(limit) = index(LIMIT);

    match value.len().cmp(&limit) {
      Ordering::Greater => Err(de::Error::custom(
        "the recorded text exceeds its byte limit",
      )),
      Ordering::Equal | Ordering::Less => Ok(Self(value)),
    }
  }
}

pub(crate) fn read<T: de::DeserializeOwned>(path: &Path, limit: u32) -> Result<Option<T>> {
  let file = match File::open(path) {
    Ok(file) => file,
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(None),
      None => return Err(Box::new(fault)),
    },
  };
  let bytes = console_core_bounded_reads::bytes(file, limit)?;
  serde_json::from_slice(&bytes).map(Some).map_err(Box::from)
}
