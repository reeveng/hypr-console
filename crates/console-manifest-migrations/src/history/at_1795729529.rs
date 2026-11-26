//! Retire the legacy executable check before systemd starts Console's idle service.
//! Installations that already removed hypridle have recorded that migration.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
  moment: Moment(1_795_729_529),
  says: "moving the old hypridle condition out of the Console idle service",
  steps: &[Step::Attic(
    "/etc/systemd/user/console-idle.service.d/require-hypridle.conf",
  )],
};
