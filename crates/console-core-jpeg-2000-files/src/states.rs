//! The named states a decoder or container reports.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
  AlreadyPresent,
  Installed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Significance {
  Insignificant,
  Significant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroCoding {
  NotCoded,
  Coded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextStripeNeighbor {
  Absent,
  Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precincts {
  Default,
  Explicit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SopMarkers {
  Excluded,
  Permitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EphMarkers {
  Absent,
  Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderEnd {
  Pending,
  Reached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StuffBit {
  Absent,
  Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaChannel {
  Absent,
  Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationMode {
  Lenient,
  Strict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteMode {
  ResolveColors,
  KeepIndices,
}
