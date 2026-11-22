//! Stream clocks round to the same millisecond boundary in every container.

use std::time::Duration;

use crate::fault::Malformed;

pub(crate) struct Interval {
  pub(crate) units: u64,
  pub(crate) per_second: u64,
}

pub(crate) fn rounded(interval: Interval) -> Result<Duration, Malformed> {
  let numerator = interval
    .units
    .saturating_mul(1000)
    .saturating_add(interval.per_second.wrapping_div(2));
  let milliseconds = numerator
    .checked_div(interval.per_second)
    .ok_or(Malformed::Properties)?;

  Ok(Duration::from_millis(milliseconds))
}
