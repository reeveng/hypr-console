//! The way in: what holds the console before anybody has logged in.
//!
//! **greetd's daemon, and only the half this machine uses.** greetd is a daemon
//! that owns the VT and the PAM conversation, and a greeter that draws and
//! talks to it over a socket; that seam is the right one and is kept. What is
//! not kept is everything that exists so that anybody's greeter can plug in --
//! the public socket, its JSON, the config language, the choice of greeter --
//! because there is one greeter and it is this tree's. It is the daemon's own
//! child, and the two talk over its stdin and stdout in [`protocol`]'s lines.
//! tokio went with the socket: one greeter and one session at a time is a
//! loop, not a runtime.
//!
//! **The machine boots into the desktop, and the greeter is where it comes
//! back to.** Game Mode is a session switch -- `steamos-session-select` names
//! the next session and restarts the login -- so the first login of a boot is
//! the one nobody types, as it was under plasmalogin. The greeter is what is
//! on the screen when a session ends, whether somebody logged out or the
//! desktop fell, and it is where the pattern is asked.
//!
//! **Each session is a process of its own, and PAM's session stays here.** PAM
//! reads an exec from the process that opened a session as that session
//! ending, so the session is opened in this process and the desktop is started
//! in a child that drops to the person before it runs anything -- which is
//! greetd's shape exactly, less the worker process greetd puts between: with
//! one session at a time, systemd restarting this unit is the isolation the
//! worker was for.

pub mod protocol;
pub mod sessions;
pub mod stored_pattern;
pub mod system;
