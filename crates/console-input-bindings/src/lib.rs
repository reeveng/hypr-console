//! What a job is bound to, on any input this machine has.
//!
//! This was `console_input_gamepad::jobs`, and it was held there "because both
//! ends need it and neither owns it". That stopped being true the moment a job
//! could be reached from a keyboard as well as from the pad: a crate that is a
//! Legion Go you can press is the wrong place to keep what Super and I mean.
//! So the shape of an answer lives here, and each input's own vocabulary stays
//! with the thing that understands it -- `console_input_gamepad::vocabulary`
//! for what is on the front of the machine, [`keys`] for what is under
//! someone's fingers.
//!
//! A binding is an input, whatever is held, and the one thing pressed. It is
//! written the way it is said -- `l2 + right-paddle-bottom` on the pad, and
//! `keyboard: super + i` -- and read back the same way. What it is *not* is a
//! mechanism: which
//! program carries the press out is worked out from the input and never
//! written down here, because a person moving a job onto a key is not choosing
//! between a daemon and a compositor.
//!
//! The other half is [`moved`], which is the file in someone's home holding
//! only what they moved. Both halves are shared by four programs -- the daemon
//! matches presses against them, the setup screen writes them, the card reads a
//! press into one, and the guide says them out loud -- and a copy of either in
//! any of the four would be the copy that drifts.

use std::fmt;
use std::path::PathBuf;

pub mod active;
pub mod bound;
pub mod keys;
pub mod moved;

pub use bound::{Binding, Fits, Input, NOTHING, Played};
pub use moved::{Tasks, Moved, NAMED, Rebound, path_in};

#[derive(Debug)]
pub enum Unbound {
    NoSuchKey(String),
    NoSuchInput(String, String),
    ATrigger(String, String),
    NotOnThisMachine(String),
    AModifier(String, String),
    AKey(String),
    Untabled(toml::de::Error),
    UnderAJob(String, Box<Unbound>),
    Rootless,
    Holding(PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
}

impl fmt::Display for Unbound {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unbound::NoSuchKey(word) => write!(to, "no key called {word:?}"),
            Unbound::NoSuchInput(word, said) => {
                write!(to, "nothing here is called {word:?}, in {said:?}")
            }
            Unbound::ATrigger(pressed, binding) => write!(
                to,
                "{pressed:?} is a trigger, and a trigger is what is held: {binding}"
            ),
            Unbound::NotOnThisMachine(word) => {
                write!(to, "nothing on this machine is called {word:?}")
            }
            Unbound::AModifier(pressed, binding) => write!(
                to,
                "{pressed:?} is a modifier, and a modifier is what is held: {binding}"
            ),
            Unbound::AKey(word) => write!(
                to,
                "{word:?} is a key, and a key is what is pressed rather than held"
            ),
            Unbound::Untabled(fault) => {
                write!(to, "the button table does not parse: {fault}")
            }
            Unbound::UnderAJob(job, fault) => write!(to, "{job}: {fault}"),
            Unbound::Rootless => write!(to, "the remembered input has no directory"),
            Unbound::Holding(under, fault) => write!(to, "{}: {fault}", under.display()),
            Unbound::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unbound {}

impl From<console_core_atomic_writes::Unwritten> for Unbound {
    fn from(fault: console_core_atomic_writes::Unwritten) -> Self {
        Unbound::Writing(fault)
    }
}
