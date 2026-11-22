#[derive(Debug)]
pub(crate) enum Failure {
  Xml(crate::Error),
  Assertion { line: u32 },
  MissingValue { line: u32 },
  UnexpectedError { line: u32 },
  UnexpectedValue { line: u32 },
}

impl From<crate::Error> for Failure {
  fn from(error: crate::Error) -> Self {
    Self::Xml(error)
  }
}

enum Comparison {
  Equal,
  Different,
}

impl From<bool> for Comparison {
  fn from(equal: bool) -> Self {
    match bool::then_some(equal, ()) {
      Some(()) => Self::Equal,
      None => Self::Different,
    }
  }
}

pub(crate) fn equal<T: PartialEq<U> + ?Sized, U: ?Sized>(
  left: &T,
  right: &U,
  line: u32,
) -> Result<(), Failure> {
  match Comparison::from(left == right) {
    Comparison::Equal => Ok(()),
    Comparison::Different => Err(Failure::Assertion { line }),
  }
}

pub(crate) trait Extract {
  type Value;
  type Error;

  fn value(self, line: u32) -> Result<Self::Value, Failure>;

  fn error(self, line: u32) -> Result<Self::Error, Failure>;
}

impl<T> Extract for Option<T> {
  type Value = T;
  type Error = ();

  fn value(self, line: u32) -> Result<T, Failure> {
    match self {
      Some(value) => Ok(value),
      None => Err(Failure::MissingValue { line }),
    }
  }

  fn error(self, line: u32) -> Result<(), Failure> {
    match self {
      Some(_) => Err(Failure::UnexpectedValue { line }),
      None => Ok(()),
    }
  }
}

impl<T, E> Extract for Result<T, E> {
  type Value = T;
  type Error = E;

  fn value(self, line: u32) -> Result<T, Failure> {
    match self {
      Ok(value) => Ok(value),
      Err(_unexpected_fault) => Err(Failure::UnexpectedError { line }),
    }
  }

  fn error(self, line: u32) -> Result<E, Failure> {
    match self {
      Ok(_unexpected_value) => Err(Failure::UnexpectedValue { line }),
      Err(error) => Ok(error),
    }
  }
}
