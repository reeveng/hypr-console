//! Reproducible samples use a permutation of a power-of-two integer domain.

use console_core_never::Never;
use core::num::NonZeroU32;

#[derive(Clone, Copy)]
pub struct Sample {
  pub count: u32,
  pub density: u32,
}

#[derive(Debug)]
pub struct InvalidSample;

impl core::fmt::Display for InvalidSample {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    output.write_str("sample size and density must define a nonzero power-of-two domain")
  }
}

impl std::error::Error for InvalidSample {}

impl core::fmt::Display for Sample {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(output, "{}/{}", self.count, self.density)
  }
}

impl Sample {
  pub fn limit(&self) -> Result<NonZeroU32, InvalidSample> {
    let bound = self.count.checked_mul(self.density).ok_or(InvalidSample)?;

    NonZeroU32::new(bound)
      .filter(|limit| limit.is_power_of_two() && self.count <= limit.get())
      .ok_or(InvalidSample)
  }

  pub fn values(&self) -> Result<impl Iterator<Item = u32> + use<>, InvalidSample> {
    let limit = self.limit()?;

    Ok((0..self.count).map(move |value| value.wrapping_mul(2_654_435_761) % limit))
  }
}

pub fn parameters() -> Result<impl Iterator<Item = Sample>, Never> {
  Ok([2, 8, 64, 512].into_iter().flat_map(|density| {
    [1024, 4096, 32_768, 65_536]
      .into_iter()
      .map(move |count| Sample { count, density })
  }))
}
