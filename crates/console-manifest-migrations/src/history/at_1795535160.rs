//! Remove the former Night Light package after the Rust service starts.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
  moment: Moment(1_795_535_160),
  says: "removing hyprsunset after deploying Console Night Light",
  steps: &[
    Step::RemovePackage("hyprsunset"),
    Step::Attic("/home/@user@/.config/console/hypr/hyprsunset.conf"),
  ],
};
