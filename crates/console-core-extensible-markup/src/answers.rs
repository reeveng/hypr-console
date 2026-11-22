//! The categories the XML tree and tokenizer can report.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
  Reached,
  Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMatch {
  Matches,
  Different,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consumption {
  Consumed,
  Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStart {
  Valid,
  Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
  Present,
  Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMatch {
  Matches,
  Different,
}

impl From<bool> for NodeMatch {
  fn from(matches: bool) -> Self {
    match bool::then_some(matches, ()) {
      Some(()) => Self::Matches,
      None => Self::Different,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dtd {
  Allow,
  Deny,
}
