//! Remove the former idle configuration after Console's idle service is installed.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
  moment: Moment(1_795_621_878),
  says: "removing hypridle and the separate lock power watcher",
  steps: &[
    Step::Stop("console-lock-power.service"),
    Step::Disable("console-lock-power.service"),
    Step::Attic("/etc/systemd/user/console-lock-power.service"),
    Step::Attic("/etc/console/hypridle.conf"),
    Step::RemovePackage("hypridle"),
  ],
};
