//! Set up the packaged multi-user Nix daemon after the package is installed.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
  moment: Moment(1_795_614_091),
  says: "setting up the multi-user Nix daemon",
  steps: &[Step::SetupNixDaemon],
};
