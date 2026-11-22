//! Control value table.

use console_core_never::Never;

use super::{F26Dot6, cow_slice::CowSlice, error::HintErrorKind};

pub struct Cvt<'a>(CowSlice<'a>);

impl Cvt<'_> {
  pub fn get(&self, index: u64) -> Result<F26Dot6, HintErrorKind> {
    self
      .0
      .get(index)
      .map(|bits| {
        let Ok(value) = F26Dot6::from_bits(bits);
        value
      })
      .map_err(|_out_of_bounds| HintErrorKind::InvalidCvtIndex(index))
  }

  pub fn set(&mut self, index: u64, value: F26Dot6) -> Result<(), HintErrorKind> {
    self
      .0
      .set(index, {
        let Ok(value_2) = value.to_bits();
        value_2
      })
      .map_err(|_out_of_bounds| HintErrorKind::InvalidCvtIndex(index))
  }

  pub fn len(&self) -> Result<u64, Never> {
    self.0.len()
  }
}

impl<'a> From<CowSlice<'a>> for Cvt<'a> {
  fn from(value: CowSlice<'a>) -> Self {
    Self(value)
  }
}
