//! Interpreter states returned to the outline scaler.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionPresence {
  Present,
  Absent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
  Enabled,
  Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackwardCompatibility {
  Enabled,
  Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointBounds {
  Within,
  Outside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveKind {
  On,
  Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GlyphKind {
  #[default]
  Simple,
  Composite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pedantry {
  Strict,
  Lenient,
}

impl From<bool> for Pedantry {
  fn from(enabled: bool) -> Self {
    match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => Self::Strict,
      ::core::option::Option::None => Self::Lenient,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpolationProgress {
  Applied,
  Pending,
}

impl From<bool> for InterpolationProgress {
  fn from(enabled: bool) -> Self {
    match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => Self::Applied,
      ::core::option::Option::None => Self::Pending,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomaticFlip {
  Enabled,
  Disabled,
}

impl From<bool> for AutomaticFlip {
  fn from(enabled: bool) -> Self {
    match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => Self::Enabled,
      ::core::option::Option::None => Self::Disabled,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
  Rotated,
  Unrotated,
}

impl From<bool> for Rotation {
  fn from(enabled: bool) -> Self {
    match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => Self::Rotated,
      ::core::option::Option::None => Self::Unrotated,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stretch {
  Stretched,
  Unstretched,
}

impl From<bool> for Stretch {
  fn from(enabled: bool) -> Self {
    match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => Self::Stretched,
      ::core::option::Option::None => Self::Unstretched,
    }
  }
}

impl From<bool> for BackwardCompatibility {
  fn from(enabled: bool) -> Self {
    match ::core::primitive::bool::then_some(enabled, ()) {
      ::core::option::Option::Some(()) => Self::Enabled,
      ::core::option::Option::None => Self::Disabled,
    }
  }
}
