//! The categories returned by this crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShouldPreScaleCoverageAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsOpaqueAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsVerticalAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickRejectAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortIncreasingYAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChopMonoQuadAtXAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChopMonoQuadAtYAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChopMonoQuadAtAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooBigForReliableFloatMathAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanConvertToFdot16Answer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsBetweenUnsortedAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntersectAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedLtAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainsNoEmptyCheckAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNotMonotonicAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChopMonoCubicAtXAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChopMonoCubicAtYAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicDchopAtInterceptAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnPtrEqAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometricOverlapAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometricContainsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickCubicNicenessCheckAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lt90Answer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YInRectAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XInRectAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushStagesAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsFocalOnCircleAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsWellBehavedAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNativelyFocalAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsSolidColorAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsTooBigForMathAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredAnswer {
  Yes,
  No,
}


#[derive(Clone, Copy)]
pub(crate) enum PointOrder {
  Forward,
  Reversed,
}

impl PointOrder {
  pub(crate) fn reversed(self) -> Result<Self, console_core_never::Never> {
    Ok(match self {
      Self::Forward => Self::Reversed,
      Self::Reversed => Self::Forward,
    })
  }
}
