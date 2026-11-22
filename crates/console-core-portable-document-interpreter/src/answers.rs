//! The words this crate says where a `bool` used to stand, for an answer it gives and for a choice
//! a caller makes. Each is two named outcomes, so a reader of a call site sees which one it is.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CffProgram {
  Cff,
  NotCff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CidKeying {
  CidKeyed,
  NameKeyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Containment {
  Inside,
  Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraySpace {
  DeviceGray,
  NotDeviceGray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSize {
  Decodable,
  TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexedSpace {
  Indexed,
  NotIndexed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
  Smooth,
  Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabProfile {
  Lab,
  NotLab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Masking {
  Masked,
  Unmasked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nesting {
  Entered,
  TooDeep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalContent {
  Begun,
  Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintOperation {
  Stroke,
  Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintBudget {
  Granted,
  Spent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternSpace {
  Pattern,
  NotPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serifs {
  Serif,
  SansSerif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slant {
  Italic,
  Upright,
}

impl From<bool> for Slant {
  fn from(italic: bool) -> Self {
    match italic.then_some(()) {
      Some(()) => Self::Italic,
      None => Self::Upright,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
  Monospace,
  Proportional,
}

impl From<bool> for Spacing {
  fn from(monospace: bool) -> Self {
    match monospace.then_some(()) {
      Some(()) => Self::Monospace,
      None => Self::Proportional,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrgbProfile {
  Srgb,
  NotSrgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbolism {
  NonSymbolic,
  Symbolic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type1Program {
  Type1,
  NotType1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
  Visible,
  Hidden,
}

impl From<bool> for Visibility {
  fn from(visible: bool) -> Self {
    match visible.then_some(()) {
      Some(()) => Self::Visible,
      None => Self::Hidden,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
  Bold,
  Regular,
}

impl From<bool> for Weight {
  fn from(bold: bool) -> Self {
    match bold.then_some(()) {
      Some(()) => Self::Bold,
      None => Self::Regular,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritingMode {
  Horizontal,
  Vertical,
}
