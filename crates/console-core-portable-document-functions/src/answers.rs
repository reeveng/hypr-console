//! The categories returned by this crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsBoolAnswer {
  Yes,
  No,
}

impl From<bool> for AsBoolAnswer {
  fn from(truth: bool) -> Self {
    match bool::then_some(truth, ()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

impl AsBoolAnswer {
  pub(crate) fn as_number(self) -> Result<f32, console_core_never::Never> {
    Ok(match self {
      Self::Yes => 1.0,
      Self::No => 0.0,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}
