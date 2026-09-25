//! Sixteen programs, a unit and a menu entry, under the names they had before
//! `[build]` had a rule.
//!
//! A binary in `[build]` is installed into /usr/local/bin under its own name, and
//! an apply installs a name and has never removed one. So a machine that applied
//! the commit before this one has both spellings of every renamed program sitting
//! beside each other, and -- this is what makes it worth a migration rather than
//! a tidy -- the old one still runs. It is the same code compiled a day earlier.
//! Nothing tells a person which of the two they have, and nothing here would ever
//! say the old one is wrong, because it is not: it is right and it is stale, and
//! those look identical from a prompt.
//!
//! The unit is the half that bites on its own. `console-sky.service` is enabled
//! and pulled in by console.target, and the wallpaper's unit is
//! `console-wallpaper.service` now. An apply that installs the new one leaves the
//! old one enabled beside it: two daemons choosing a picture for one screen, each
//! writing over the other every time the weather moves. Stopping comes before
//! disabling for the reason the earlier unit rename wrote down -- `disable` takes
//! the unit out of the wants and leaves the process running -- and `console apply`
//! runs its migrations before any section, so this is the moment there is one of
//! each.
//!
//! The menu entry is smaller and is the one someone sees. `console-music.desktop`
//! is what mimeapps.list points at now; `console-music-panel.desktop` is still in
//! /usr/share/applications, still names a program that still exists, and so still
//! draws a second Music in the launcher, identical to the first.
//!
//! What is deliberately not swept is the four the rename did not touch:
//! `bar-door` and `bar-updating` changed which crate builds them and kept their
//! names, so nothing about them left the manifest and nothing is left behind.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1789413352),
    says: "standing down the wallpaper and sweeping the sixteen programs named before [build] had a rule",
    steps: &[
        Step::Disable("console-sky.service"),
        Step::Attic("/etc/systemd/user/console-sky.service"),
        Step::Attic("/usr/local/bin/console-sky"),
        Step::Attic("/usr/local/bin/desktop-mode"),
        Step::Attic("/usr/local/bin/dictate"),
        Step::Attic("/usr/local/bin/download-find"),
        Step::Attic("/usr/local/bin/download-get"),
        Step::Attic("/usr/local/bin/download-panel"),
        Step::Attic("/usr/local/bin/game-mode"),
        Step::Attic("/usr/local/bin/game-return"),
        Step::Attic("/usr/local/bin/layout-panel"),
        Step::Attic("/usr/local/bin/notices-panel"),
        Step::Attic("/usr/local/bin/one-format"),
        Step::Attic("/usr/local/bin/put-away"),
        Step::Attic("/usr/local/bin/sky-press"),
        Step::Attic("/usr/local/bin/stick-scroll"),
        Step::Attic("/usr/local/bin/switch-language"),
        Step::Attic("/usr/local/bin/virtual-keyboard"),
        Step::Attic("/usr/share/applications/console-music-panel.desktop"),
    ],
};
