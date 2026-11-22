//! A test that meets an absent value says so through its result rather than by panicking.

#[derive(Debug)]
pub(crate) enum TestFailure {
  MissingValue,
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
