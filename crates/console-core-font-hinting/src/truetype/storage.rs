//! Storage area.

use super::{cow_slice::CowSlice, error::HintErrorKind};

pub struct Storage<'a>(CowSlice<'a>);

impl Storage<'_> {
  pub fn get(&self, index: u64) -> Result<i32, HintErrorKind> {
    self
      .0
      .get(index)
      .map_err(|_out_of_bounds| HintErrorKind::InvalidStorageIndex(index))
  }

  pub fn set(&mut self, index: u64, value: i32) -> Result<(), HintErrorKind> {
    self
      .0
      .set(index, value)
      .map_err(|_out_of_bounds| HintErrorKind::InvalidStorageIndex(index))
  }
}

impl<'a> From<CowSlice<'a>> for Storage<'a> {
  fn from(value: CowSlice<'a>) -> Self {
    Self(value)
  }
}
