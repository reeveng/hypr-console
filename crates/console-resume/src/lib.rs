//! What was open when the desktop went away, put back, and kept current.
//!
//! A fork of `hyprsession`, carried here as source rather than as a binary
//! because it is the only program on this device that closes other people's
//! windows and it should be readable by whoever has to trust it.
//! `crates/console-manifest-publish/papers/forks.md` says where it came from and
//! what the fork changed; this is what the port changed.
//!
//! Everything that talks to the compositor goes through `console-compositor`.
//! The fork opened the IPC socket itself and had a second copy of where that
//! socket lives, and it read the event socket's words in its own vocabulary --
//! which is the third and fourth copy of a thing this tree keeps in one place.
//! `console_compositor::stirred` is where those words are now, and this crate
//! spells only what a *session* means by them.
//!
//! What upstream still has open, this answers rather than works around. A
//! special workspace is known by the name it was given and not by the number
//! -99, which is only ever the first of them. A window that floats comes back
//! at the size it had, and a tiled one is left where the layout puts it rather
//! than argued with in pixels. A session file that will not be read is a fault
//! and not a session with nothing in it, which is what turned one bad file into
//! a swept desktop. What is not answered is grouped windows: nothing on this
//! device groups any, a window's group is not something `console-compositor`
//! carries, and the dispatcher that would rebuild one takes a direction rather
//! than a window -- so it is a feature with a question inside it rather than a
//! fix somebody is owed.
//!
//! What it asks of the machine besides the compositor is `/proc`: a window's
//! process says what started it, and a terminal's process tree says what was
//! going on inside it. Nothing here spawns a program to find something out.

pub mod already;
pub mod lua;
pub mod session;
pub mod starting;
pub mod terminal;

pub const OURS: &str = "resume";
