//! The four installed names the crates' rename took with it.
//!
//! `console-controller` became `console-input-controller` and `console-keyboard`
//! became `console-input-keyboard`, so the units those two run under, the drop-in
//! beside one of them, and the two desktop entries named after `console-viewer`
//! and `console-download` are all installed under names the manifest has stopped
//! saying. Everything else in that rename was a crate, which is a directory here
//! and nothing at all on a machine.
//!
//! The two units are why this is not tidying. A machine that applied the commit
//! before this one has both of them enabled and pulled in by console.target, and
//! an apply that installs the new pair leaves the old pair beside it: two daemons
//! reading the same pad, two keyboards answering the same key.
//!
//! Stopping is the half that is easy to forget. `disable` takes the unit out of
//! the wants and leaves the process running, and the programs behind these two
//! units did not change their names -- only the units did -- so what is running
//! would keep running against the same binary the new unit starts a second copy
//! of. `console apply` runs its migrations before any section, so this is the
//! moment there is one of each: stop, then disable, then move the files.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1788696995),
    says: "stopping and disabling the pair the rename replaced, and sweeping what the manifest stopped naming",
    steps: &[
        Step::Disable("console-controller.service"),
        Step::Disable("console-keyboard.service"),
        Step::Attic("/etc/systemd/user/console-controller.service"),
        Step::Attic("/etc/systemd/user/console-controller.service.d/opening.conf"),
        Step::Attic("/etc/systemd/user/console-controller.service.d"),
        Step::Attic("/etc/systemd/user/console-keyboard.service"),
        Step::Attic("/usr/share/applications/console-download.desktop"),
        Step::Attic("/usr/share/applications/console-viewer.desktop"),
    ],
};
