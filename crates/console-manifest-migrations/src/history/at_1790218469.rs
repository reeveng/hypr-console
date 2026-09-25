//! The files became an app and the program was named for what somebody types,
//! `files`, as books is. The old name was a panel's: drawn by the host, one of
//! the pickers taking turns, closed by whatever opened next.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1790218469),
    says: "sweeping the files panel, which is the files app now",
    steps: &[
        Step::Attic("/usr/local/bin/files-panel"),
    ],
};
