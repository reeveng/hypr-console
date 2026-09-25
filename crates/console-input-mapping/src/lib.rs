//! Where the buttons are moved, on a device that is not the one this grew on.
//!
//! Every default in this repository is written in a Legion Go's words. Five of
//! them -- the four paddles and Legion right -- name nothing on an ordinary
//! pad, and the menu, closing, dictation, the screenshot and the settings sit
//! on all five. On such a device this desktop installs, works, and has five
//! promises it cannot keep, which is what `console check` says and what the
//! notification after an apply says.
//!
//! This is where someone answers. One row per thing the desktop does, what
//! plays it now beside it, and moving one is pressing the button you want it
//! on -- with a trigger held first, if you want it on a layer. NoOne holding
//! a handheld knows which paddle `RightPaddle3` is, and a list of names is the
//! worse screen for the same question.
//!
//! `rows` is the screen and has never seen a machine. `mapping-panel` is the
//! screen with one, and `console-asking` is the card that reads the press.

pub mod card;
pub mod update;
pub mod rows;
pub mod table;

pub use card::{WHO, card, door};

#[derive(Debug)]
pub enum Unmapped {
    NoOnes,
    Holding(std::path::PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unmapped {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unmapped::NoOnes => {
                write!(to, "can't identify this controller")
            }
            Unmapped::Holding(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unmapped::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unmapped {}
