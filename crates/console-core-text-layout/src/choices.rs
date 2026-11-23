//! Choices at the boundaries of shaping and editing.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pruning {
  Discard,
  Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redraw {
  Needed,
  Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundarySide {
  Before,
  After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
  Forward,
  Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiters {
  Included,
  Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modification {
  Different,
  Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyHandling {
  PassThrough,
  Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
  RightToLeft,
  LeftToRight,
}

impl From<unicode_bidi::Direction> for TextDirection {
  fn from(direction: unicode_bidi::Direction) -> Self {
    match direction {
      unicode_bidi::Direction::RightToLeft => Self::RightToLeft,
      unicode_bidi::Direction::LeftToRight => Self::LeftToRight,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
  Blank,
  Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunAlignment {
  Congruent,
  Incongruent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
  Present,
  Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoration {
  Decorated,
  Plain,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DecorationLine {
  #[default]
  Hidden,
  Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
  Compatible,
  Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
  Monospaced,
  Proportional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reshaping {
  Needed,
  Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
  Exceeds,
  Within,
}
