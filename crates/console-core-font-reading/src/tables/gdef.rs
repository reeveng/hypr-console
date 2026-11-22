//! the [GDEF] table
//!
//! [GDEF]: https://docs.microsoft.com/en-us/typography/opentype/spec/gdef

pub use super::layout::{ClassDef, CoverageTable, DeviceOrVariationIndex};

use super::variations::ItemVariationStore;

include!("../../generated/generated_gdef.rs");
