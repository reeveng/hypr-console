//! What a program decided, said without being carried out.
//! `console_input_controller::doing` is where this was written once already,
//! for one daemon, and it is the most reliable thing in this tree. What is here
//! is the same move made for every program: a decision that is a value can be
//! compared, printed, kept in a list and asserted on, and none of those are
//! things a decision that has already happened can be.  There is a second
//! reason nobody wrote down until confinement was being argued about. A program
//! that says what it wants done rather than doing it is a program whose effects
//! can be held against a declaration *before* they are carried out, which makes
//! the loop that carries them out the one thing that has to be trusted instead
//! of every program. That is free once the programs are functions, and it is
//! the only path to confining the bar -- which needs to start whatever a person
//! chose, and so cannot be given a narrow enough list of its own.
//! [`Doing::Watch`] is [`Doing::Ask`] with nothing between the program and the
//! screen. The difference is not tidiness: `just ready` and a cargo build over
//! ssh are minutes long, and a run whose output is captured is a run that
//! shows a person nothing until it has finished. What comes back is how it
//! went and no words, because the words already went somewhere a program
//! cannot read them.
//!
//! [`Doing::AskWhoever`] is the one doing here that waits on a person. It is a
//! question at the terminal of whoever started the program, and the answer is
//! a [`Chose`] rather than a line: a program that reads what somebody typed is
//! a program with a parser in it, and every use of this so far wanted yes or
//! no. [`Question::silence`] is what an empty line means, which is the
//! difference between `[y/N]` and `[Y/n]` and is the only part of the prompt
//! that decides anything.

use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::wants::Wants;
use crate::word::Chose;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doing<Does> {
    Ask(Runs),
    Watch(Runs),
    AskWhoever(Question),
    Start(Runs),
    Listen(Wants),
    Deafen(Wants),
    Write(Writing),
    Say(Saying),
    Print(String),
    Stop(Ending),
    Its(Does),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runs {
    pub program: Named,
    pub argv: Vec<String>,
}

impl Runs {
    pub fn theirs(program: Program, argv: &[&str]) -> Result<Self, Never> {
        let Ok(argv) = worded(argv);

        Ok(Runs { program: Named::Theirs(program), argv })
    }

    pub fn ours(program: &'static str, argv: &[&str]) -> Result<Self, Never> {
        let Ok(argv) = worded(argv);

        Ok(Runs { program: Named::Ours(program), argv })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Theirs(Program),
    Ours(&'static str),
}

impl Named {
    pub const fn name(self) -> Result<&'static str, Never> {
        match self {
            Named::Theirs(program) => program.name(),
            Named::Ours(name) => Ok(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub asks: String,
    pub silence: Chose,
}

impl Question {
    pub fn unless(asks: &str, silence: Chose) -> Result<Self, Never> {
        Ok(Question { asks: asks.to_string(), silence })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writing {
    pub at: PathBuf,
    pub what: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saying {
    pub says: String,
    pub more: Option<String>,
}

impl Saying {
    pub fn says(said: &str) -> Result<Self, Never> {
        Ok(Saying { says: said.to_string(), more: None })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    Done,
    Badly(String),
}

fn worded(argv: &[&str]) -> Result<Vec<String>, Never> {
    Ok(argv.iter().map(|word| (*word).to_string()).collect())
}
