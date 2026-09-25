//! The startup check and the crash reporter, under the words they were.
//!
//! `console well` is `console health`, and `console-fell` is
//! `console-report-crash`. Neither does anything it did not do before: one asks
//! a machine that has just come up whether it is what the manifest says and
//! whether every piece of it is up, and the other is what every unit runs as it
//! stops so a service that died says so on the screen. What changed is that both
//! were named in a word this tree uses for something else -- a bar tone, a
//! hundred sentences of ordinary prose -- and neither name said its job to
//! anybody who had not already read it. The rest of the world calls the first a
//! health check and the second a crash reporter, and Apple ships the second
//! under very nearly this name.
//!
//! What that leaves on a machine is the whole of the old half, running. The
//! timer is enabled, so it goes on pulling `console-well.service` in at every
//! hour, and that unit's ExecStart is `/usr/local/bin/console well` -- a
//! subcommand the engine no longer has, so what it now does every hour is print
//! the help and exit 2. The unit takes it quietly (its ExecStart carries the
//! leading dash it always did), which is the worst of the two ways this could
//! go: nothing says the hourly check has stopped happening, and the new one is
//! already running beside it under its own timer. The check that exists to say
//! when something on this machine is quietly not working is not the one to leave
//! quietly not working.
//!
//! `console-fell` is the other shape. Nothing starts it any more -- every unit's
//! ExecStopPost names the new binary after an apply -- and a program nothing
//! starts is only a file, but it is a file that still raises cards from
//! `$XDG_RUNTIME_DIR/console` under counts the live reporter keeps separately,
//! so anyone who does start it gets a second reporter counting its own way.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1789954335),
    says: "standing down the startup check and sweeping the crash reporter under their old names",
    steps: &[
        Step::DisableGlobally("console-well.timer"),
        Step::Attic("/etc/systemd/user/console-well.timer"),
        Step::Attic("/etc/systemd/user/console-well.service"),
        Step::Attic("/usr/local/bin/console-fell"),
    ],
};
