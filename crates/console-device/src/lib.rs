//! The handheld, reached from this checkout.
//!
//! Two programs live here and both are run on a laptop rather than on the
//! device: one puts this checkout onto the machine and brings it to match, and
//! one moves a machine that still carries the old names over to the new ones.
//! Neither is part of the desktop and neither is in `[build]` -- the device
//! never runs them, it is what they are pointed at.
//!
//! They are together because they are one subject. What travels between here
//! and the device is a history: `console apply` on the machine and `console
//! save` on the machine are the far end of a push and a fetch, and a deploy
//! that did not first ask what the device has is a push that fails days later
//! in git's vocabulary rather than the machine's.
//!
//! `console-pull` is the third of them and is not here yet. It went into
//! `console-repository` while this was being written, and moving it is two
//! lines whenever that lands.
//!
//! Nothing in either program touches a machine. Both are
//! `console_program_contract::Program`s, so what a deploy would do to a device
//! is a list of values a test on a laptop can read -- which is the half of
//! this that a shell script could never be asked for, and the reason the lock,
//! the two moments the tree is looked at, and every refusal below have tests
//! at all.

pub mod deploying;
pub mod migrating;
pub mod naming;
