//! Everything that can reach a program.
//!
//! Six of these and no others, which is the point: a program that is a
//! function of its words does not rot when something else on the machine
//! changes, because what arrives is either a word it knows or a word it does
//! not.
//!
//! [`Changed`] carries a source's line untouched: the program that asked to
//! hear a topic is the one that knows what its words mean.
//!
//! There is no `Word::Now`. A clock reaching a program through the side of it
//! is the same fault as a file reaching it through the side of it -- two runs
//! of the same transcript would answer differently and nobody could say why.
//! Time arrives as [`Word::CameRound`], because a program that wants to know
//! what time it is has to have asked for a [`crate::Wants`] that says when.
//!
//! What it is told is how long it has been up, and not what the clock says.
//! Every program here that has ever compared two moments was comparing two
//! readings of the same monotonic count -- `console_input_controller::clock` is
//! `since_boot` for exactly that reason -- so a wall clock in this word would
//! be a thing nobody wanted and a thing that goes backwards twice a year. A
//! `Duration` also keeps `Word` comparable, which an `f64` would not: a
//! transcript is a list of words held against another list, and two moments
//! that are equal have to say so.
//!
//! [`Chose`] is what a person answered, and it carries neither the question
//! nor the words they typed. A program that had two questions outstanding
//! could not tell the answers apart, and that is the right shape rather than a
//! missing field: the loop puts one question and waits for it, because a
//! second one printed under the first is two prompts and one line of input.
//! Which question was asked is the state the program was in when it asked.

use std::path::PathBuf;

use crate::doing::Runs;
use crate::wants::Round;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Word<Hears> {
    Opened,
    Changed(Changed),
    CameRound(Round, Since),
    Answered(Answer),
    Chose(Chose),
    Stopping,
    Its(Hears),
}

pub type Since = std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub about: Topic,
    pub said: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Topic {
    Compositor,
    Sound,
    Network,
    Notices,
    Units,
    Player,
    Path(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub ran: Runs,
    pub said: String,
    pub went: Went,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chose {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    Well,
    Badly(Option<i32>),
}
