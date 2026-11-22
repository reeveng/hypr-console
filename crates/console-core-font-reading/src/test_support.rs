//! Fixture checks return their failures to the test harness.

use alloc::{boxed::Box, format};
use core::{error::Error, fmt::Debug, panic::Location};

pub(crate) type Failure = Box<dyn Error>;

pub(crate) trait FixtureValue {
  type Value;

  fn checked(self) -> Result<Self::Value, Failure>;
}

impl<T> FixtureValue for Option<T> {
  type Value = T;

  fn checked(self) -> Result<T, Failure> {
    self.ok_or_else(|| crate::test_support::Failure::from("required fixture value is absent"))
  }
}

impl<T, E: Debug> FixtureValue for Result<T, E> {
  type Value = T;

  fn checked(self) -> Result<T, Failure> {
    self.map_err(|error| crate::test_support::Failure::from(format!("{error:?}")))
  }
}

#[track_caller]
pub(crate) fn fixture_value<T: FixtureValue>(
  outcome: T,
  description: &str,
) -> Result<T::Value, Failure> {
  let at = Location::caller();
  outcome
    .checked()
    .map_err(|error| crate::test_support::Failure::from(format!("{description} at {at}: {error}")))
}

#[track_caller]
pub(crate) fn fixture_error<T: Debug, E>(
  outcome: Result<T, E>,
  description: &str,
) -> Result<E, Failure> {
  let at = Location::caller();

  match outcome {
    Ok(value) => Err(crate::test_support::Failure::from(format!("{description} at {at}: read succeeded with {value:?}"))),
    Err(error) => Ok(error),
  }
}
