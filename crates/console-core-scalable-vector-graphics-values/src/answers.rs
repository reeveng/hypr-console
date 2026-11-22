//! The categories returned by this crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsHorizontalAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsVerticalAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsCmdAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNumberStartAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
  Reached,
  Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsCurrByteEqAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMatch {
  Matches,
  Different,
}
