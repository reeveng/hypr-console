//! The viewer became an app and the program was named for what somebody types,
//! `viewer`, as the files became `files`. The old name was a panel's: drawn by
//! the host, one of the pickers taking turns, closed by whatever opened next.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1790221016),
    says: "sweeping the viewer panel, which is the viewer app now",
    steps: &[
        Step::Attic("/usr/local/bin/viewer-panel"),
    ],
};
