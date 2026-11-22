//! The categories returned by this crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsEvenAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadTooCurvyAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicTooCurvyAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheapDistExceedsLimitAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasValidTangentAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsZeroLengthSincePointAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnSameSideAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BetweenAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicStrokeAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicMidOnLineAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreJoinToAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadStrokeAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasOnlyMoveToAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsCurrentContourEmptyAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsClockwiseAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetNormalUnitNormalAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetNormalUnitNormal2Answer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnPtrEqAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitWithStartAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitWithEndAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegenerateVectorAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadInLineAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointsWithinDistAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharpAngleAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtInQuadBoundsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicInLineAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsFiniteAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsValidAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsIdentityAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsScaleAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsSkewAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsTranslateAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsScaleTranslateAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasScaleAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasSkewAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasTranslateAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsZeroAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlmostEqualAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualsWithinToleranceAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizeAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetNormalizeAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanNormalizeAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetLengthAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetLengthFromAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetPointLengthAnswer {
  Yes,
  No,
}
