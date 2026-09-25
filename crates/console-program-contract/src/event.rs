//! Everything that can reach a program.
//!
//! Six of these and no others, which is the point: a program that is a
//! function of its events does not rot when something else on the machine
//! changes, because what arrives is either an event it knows or one it does
//! not.
//!
//! [`Change`] carries a source's line untouched: the program that asked to
//! hear a topic is the one that knows what its events mean.
//!
//! There is no `Event::Now`. A clock reaching a program through the side of it
//! is the same fault as a file reaching it through the side of it -- two runs
//! of the same transcript would answer differently and no one could say why.
//! Time arrives as [`Event::Tick`], because a program that wants to know
//! what time it is has to have asked for a [`crate::Subscription`] that says when.
//!
//! What it is told is how long it has been up, and not what the clock says.
//! Every program here that has ever compared two moments was comparing two
//! readings of the same monotonic count -- `console_input_controller::clock` is
//! `since_boot` for exactly that reason -- so a wall clock in this event would
//! be a thing no one wanted and a thing that goes backwards twice a year. A
//! `Duration` also keeps `Event` comparable, which an `f64` would not: a
//! transcript is a list of events held against another list, and two moments
//! that are equal have to say so.
//!
//! [`Choice`] is what a person answered, and it carries neither the question
//! nor the words they typed. A program that had two questions outstanding
//! could not tell the answers apart, and that is the right shape rather than a
//! missing field: the loop puts one question and waits for it, because a
//! second one printed under the first is two prompts and one line of input.
//! Which question was asked is the state the program was in when it asked.

use std::path::PathBuf;

use crate::effect::Command;
use crate::subscription::Timer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<E> {
    Opened,
    Changed(Change),
    Tick(Timer, Elapsed),
    Replied(Answer),
    Chosen(Choice),
    Stopping,
    Custom(E),
}

pub type Elapsed = std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub topic: Topic,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Topic {
    Compositor,
    Sound,
    Network,
    Wifi,
    Bluetooth,
    Battery,
    Notifications,
    Units,
    Player,
    Path(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub command: Command,
    pub output: String,
    pub status: ExitStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Success,
    Failure(Option<i32>),
}
