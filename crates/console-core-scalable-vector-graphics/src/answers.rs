//! The categories returned by this crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasValidTransformAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsVisibleElementAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsValidAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessPaintAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendAttributeAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveInheritAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMatch {
  Matches,
  Different,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
  Present,
  Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsGraphicAnswer {
  Yes,
  No,
}

impl From<bool> for IsGraphicAnswer {
  fn from(applies: bool) -> Self {
    match applies.then_some(()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsGradientAnswer {
  Yes,
  No,
}

impl From<bool> for IsGradientAnswer {
  fn from(applies: bool) -> Self {
    match applies.then_some(()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsPaintServerAnswer {
  Yes,
  No,
}

impl From<bool> for IsPaintServerAnswer {
  fn from(applies: bool) -> Self {
    match applies.then_some(()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsPresentationAnswer {
  Yes,
  No,
}

impl From<bool> for IsPresentationAnswer {
  fn from(applies: bool) -> Self {
    match applies.then_some(()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsInheritableAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowsInheritValueAnswer {
  Yes,
  No,
}

impl From<bool> for AllowsInheritValueAnswer {
  fn from(applies: bool) -> Self {
    match applies.then_some(()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNonInheritableAnswer {
  Yes,
  No,
}

impl From<bool> for IsNonInheritableAnswer {
  fn from(applies: bool) -> Self {
    match applies.then_some(()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsConditionPassedAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsValidSysLangAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasInputAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreserveAlphaAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StitchTilesAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsDefaultAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmallCapsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyKerningAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsVisibleAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseShapeAntialiasingAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolateAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShouldIsolateAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasTextNodesAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasDefsNodesAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasTextPathsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasXlinkAnswer {
  Yes,
  No,
}
