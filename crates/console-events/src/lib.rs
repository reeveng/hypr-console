//! One subscription per source, and everyone else is told.
//!
//! Every panel and every bar module opens its own `pactl subscribe`, its own
//! `nmcli monitor`, its own socket to the compositor. Twenty-five orphaned
//! subscriptions were once found alive on this device, the oldest four hours
//! old. That was fixed by making each program tidy up after itself, which is a
//! fix that depends on every program remembering; this is the fix that does
//! not. There is one of each, here, and a program that goes away is dropped by
//! the thing it was subscribed to rather than by its own last wishes.
//!
//! Three copies of *where the compositor's socket is* are in this tree as this
//! is written -- `console_onscreen`, `console_wallpaper::covered` and
//! `console_panel::door` each work it out. That is the same fault as the
//! subscriptions, one layer down, and it is why this crate asks
//! `console_onscreen` rather than working it out a fourth time.
//!
//! Two things it does that no program can do for itself:
//!
//! - It **replays the last word on a topic** to whoever has just subscribed,
//!   so a panel opening knows the volume before anything changes it. That is
//!   most of what `console_panel::before` is working around today.
//! - It **drops a subscriber that has gone**, because a write to a socket
//!   no one is holding fails, and that is the moment the subscription ends.
//!   Nothing has to remember anything.
//!
//! **A program with no pool is merely slower.** It asks the machine when it
//! gets in again, because getting in is itself said as a reason to ask, and
//! the bar keeps a clock only for what no source says at all. This is a daemon
//! that can be down, and nothing here may be written as though it cannot be.
//!
//! **What is not in it.** It does not parse what a source says. A pool that
//! understood every source it relays is a pool that has to be changed whenever
//! a source says something new, and the program that asked to hear a topic is
//! the one that knows what its words mean. So a line arrives as a line, and
//! `pool` is arithmetic over who wants what.

pub mod again;
pub mod bus;
pub mod subscription;
pub mod place;
pub mod pool;
pub mod serving;
pub mod sources;
pub mod watching;
pub mod wire;

pub use pool::{Pool, Who};

#[derive(Debug)]
pub enum Unserved {
    Sessionless,
    Rootless,
    Holding(std::path::PathBuf, std::io::Error),
    Unbound(std::path::PathBuf, std::io::Error),
}

impl std::fmt::Display for Unserved {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unserved::Sessionless => write!(
                to,
                "XDG_RUNTIME_DIR: nothing says where this session keeps its sockets"
            ),
            Unserved::Rootless => write!(to, "the socket has no directory"),
            Unserved::Holding(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unserved::Unbound(at, fault) => write!(to, "{}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unserved {}
