//! The named states a decoder or container reports.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementBounds {
  Within,
  Overrun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
  Matroska,
  Webm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementLevel {
  Nested,
  Top,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterMembership {
  Outside,
  Child,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourRange {
  Limited,
  Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameAccess {
  Unavailable,
  Readable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpectralExtension {
  Disabled,
  Replicated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
  Delta,
  Keyframe,
}
