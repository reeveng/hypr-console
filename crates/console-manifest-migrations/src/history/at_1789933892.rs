//! The notification store under /run, which used to be called notices.json.
//!
//! `console-notify` keeps what it has been told in one file so that a restart
//! comes back holding the same cards rather than an empty screen. The file was
//! `notices.json` and is `notifications.json`, because a crate called
//! `console-notifications` had three words for one thing and this is the word
//! Apple writes.
//!
//! What a machine is left holding is small and it is not nothing. The file is in
//! `$XDG_RUNTIME_DIR/console`, which the kernel empties at boot, so the stale one
//! goes by itself the next time the device is turned off. Until then it sits
//! beside the live file with a day of somebody's notifications in it, and the
//! `ExecStopPost` line that used to remove it now names the new file and will
//! never touch it again. A file nothing reads and nothing deletes is exactly what
//! this directory is for.
//!
//! It claims nothing the gate asks about, because nothing left the manifest:
//! `console-notify.service` kept its name and only its body changed. What is
//! swept is every runtime directory's copy, whoever's it is, which is why the
//! step names a directory to look under rather than a path.

use crate::sweeping::{Each, Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1789933892),
    says: "sweeping the notification store under its old name",
    steps: &[
        Step::AtticEach(Each { under: "/run/user", named: "console/notices.json" }),
    ],
};
