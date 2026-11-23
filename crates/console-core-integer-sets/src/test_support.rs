//! Test expectations report absence and preserve unexpected faults through the test result.

use std::fmt::{self, Debug};

pub(crate) enum TestFailure {
  MissingValue,
  #[cfg(feature = "serde")]
  UnexpectedSuccess,
  Error(Box<dyn Debug>),
}

impl Debug for TestFailure {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      #[cfg(feature = "serde")]
      Self::UnexpectedSuccess => output.write_str("an invalid value was unexpectedly accepted"),
      Self::MissingValue => output.write_str("an expected value was absent"),
      Self::Error(fault) => output.debug_tuple("unexpected fault").field(fault).finish(),
    }
  }
}

pub(crate) trait Required {
  type Value;

  fn required(self) -> Result<Self::Value, TestFailure>;
}

impl<T> Required for Option<T> {
  type Value = T;

  fn required(self) -> Result<T, TestFailure> {
    match self {
      Some(value) => Ok(value),
      None => Err(TestFailure::MissingValue),
    }
  }
}

impl<T, E: Debug + 'static> Required for Result<T, E> {
  type Value = T;

  fn required(self) -> Result<T, TestFailure> {
    match self {
      Ok(value) => Ok(value),
      Err(fault) => Err(TestFailure::Error(Box::new(fault))),
    }
  }
}
