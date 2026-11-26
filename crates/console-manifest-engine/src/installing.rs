//! Estimate transaction progress for the apply display.
//!
//! Fetching is given the first half and installing the second. It is a
//! division, not a measurement: a package that is already in the cache is not
//! fetched at all and the first half goes by at once, and one that is fifty
//! megabytes over a phone is most of the wait. Half apiece is the estimate that
//! is wrong in both directions rather than badly wrong in one.

use console_core_never::Never;
use console_how_far::Progress;

const FETCHING: f64 = 0.5;

pub fn fetched(far: Progress) -> Result<f64, Never> {
  let Ok(part) = console_how_far::fraction(far);

  Ok(part * FETCHING)
}

pub fn overall_fraction(far: Progress) -> Result<f64, Never> {
  let Ok(part) = console_how_far::fraction(far);

  Ok(FETCHING + part * (1.0 - FETCHING))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fetching_has_the_first_half_and_writing_the_second() {
    assert_eq!(fetched(Progress { done: 0, many: 4 }), Ok(0.0));
    assert_eq!(fetched(Progress { done: 4, many: 4 }), Ok(FETCHING));
    assert_eq!(
      overall_fraction(Progress { done: 0, many: 4 }),
      Ok(FETCHING)
    );
    assert_eq!(overall_fraction(Progress { done: 4, many: 4 }), Ok(1.0));
  }

  #[test]
  fn a_stage_with_no_packages_in_it_divides_by_nothing_and_says_nothing() {
    assert_eq!(fetched(Progress { done: 0, many: 0 }), Ok(0.0));
    assert_eq!(
      overall_fraction(Progress { done: 0, many: 0 }),
      Ok(FETCHING)
    );
  }
}
