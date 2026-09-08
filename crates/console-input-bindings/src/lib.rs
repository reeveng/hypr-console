//! What a job is bound to, on any input this machine has.
//!
//! This was `console_input_gamepad::jobs`, and it was held there "because both
//! ends need it and neither owns it". That stopped being true the moment a job
//! could be reached from a keyboard as well as from the pad: a crate that is a
//! Legion Go you can press is the wrong place to keep what Super and I mean.
//! So the shape of an answer lives here, and each input's own vocabulary stays
//! with the thing that understands it -- `console_input_gamepad::vocabulary`
//! for what is on the front of the machine, [`keys`] for what is under
//! somebody's fingers.
//!
//! A binding is an input, whatever is held, and the one thing pressed. It is
//! written the way it is said -- `l2 + right-paddle-bottom` on the pad, and
//! `keyboard: super + i` -- and read back the same way. What it is *not* is a
//! mechanism: which
//! program carries the press out is worked out from the input and never
//! written down here, because a person moving a job onto a key is not choosing
//! between a daemon and a compositor.
//!
//! The other half is [`moved`], which is the file in somebody's home holding
//! only what they moved. Both halves are shared by four programs -- the daemon
//! matches presses against them, the setup screen writes them, the card reads a
//! press into one, and the guide says them out loud -- and a copy of either in
//! any of the four would be the copy that drifts.

pub mod active;
pub mod bound;
pub mod keys;
pub mod moved;

pub use bound::{Binding, Fits, Input, NOTHING, Played};
pub use moved::{Jobs, Moved, NAMED, Rebound, path_in};
