//! Validation policy for embedded font programs.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validation {
  Pedantic,
  Compatible,
}

impl From<bool> for Validation {
  fn from(pedantic: bool) -> Self {
    match ::core::primitive::bool::then_some(pedantic, ()) {
      ::core::option::Option::Some(()) => Self::Pedantic,
      ::core::option::Option::None => Self::Compatible,
    }
  }
}
