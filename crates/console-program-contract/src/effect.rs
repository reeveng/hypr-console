//! What a program decided, said without being carried out.
//! `console_input_controller::effect` is where this was written once already,
//! for one daemon, and it is the most reliable thing in this tree. What is here
//! is the same move made for every program: a decision that is a value can be
//! compared, printed, kept in a list and asserted on, and none of those are
//! things a decision that has already happened can be.  There is a second
//! reason no one wrote down until confinement was being argued about. A program
//! that says what it wants done rather than doing it is a program whose effects
//! can be held against a declaration *before* they are carried out, which makes
//! the loop that carries them out the one thing that has to be trusted instead
//! of every program. That is free once the programs are functions, and it is
//! the only path to confining the bar -- which needs to start whatever a person
//! chose, and so cannot be given a narrow enough list of its own.
//! [`Effect::Stream`] is [`Effect::Run`] with nothing between the program and the
//! screen. The difference is not tidiness: `just ready` and a cargo build over
//! ssh are minutes long, and a run whose output is captured is a run that
//! shows a person nothing until it has finished. What comes back is how it
//! went and no words, because the words already went somewhere a program
//! cannot read them.
//!
//! [`Effect::Prompt`] is the one effect here that waits on a person. It is a
//! question at the terminal of whoever started the program, and the answer is
//! a [`Choice`] rather than a line: a program that reads what someone typed is
//! a program with a parser in it, and every use of this so far wanted yes or
//! no. [`Prompt::default`] is what an empty line means, which is the
//! difference between `[y/N]` and `[Y/n]` and is the only part of the prompt
//! that decides anything.

use std::path::PathBuf;

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::subscription::Subscription;
use crate::event::Choice;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect<F> {
    Run(Command),
    Stream(Command),
    Prompt(Prompt),
    Spawn(Command),
    Subscribe(Subscription),
    Unsubscribe(Subscription),
    Write(FileWrite),
    Notify(Notification),
    Print(String),
    Stop(Exit),
    Custom(F),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: Executable,
    pub arguments: Vec<String>,
}

impl<F> Effect<F> {
    pub fn written(&self) -> Result<Option<&FileWrite>, Never> {
        Ok(match self {
            Effect::Write(writing) => Some(writing),
            Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Spawn(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_)
        | Effect::Custom(_) => None,
        })
    }

    pub fn spawned(&self) -> Result<Option<&Command>, Never> {
        Ok(match self {
            Effect::Spawn(runs) => Some(runs),
            Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Write(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_)
        | Effect::Custom(_) => None,
        })
    }
}

impl Command {
    pub fn external(program: Program, arguments: &[&str]) -> Result<Self, Never> {
        let Ok(arguments) = worded(arguments);

        Ok(Command { program: Executable::External(program), arguments })
    }

    pub fn internal(program: &'static str, arguments: &[&str]) -> Result<Self, Never> {
        let Ok(arguments) = worded(arguments);

        Ok(Command { program: Executable::Internal(program), arguments })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executable {
    External(Program),
    Internal(&'static str),
}

impl Executable {
    pub const fn name(self) -> Result<&'static str, Never> {
        match self {
            Executable::External(program) => program.name(),
            Executable::Internal(name) => Ok(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub message: String,
    pub default: Choice,
}

impl Prompt {
    pub fn unless(message: &str, default: Choice) -> Result<Self, Never> {
        Ok(Prompt { message: message.to_string(), default })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWrite {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub summary: String,
    pub body: Option<String>,
}

impl Notification {
    pub fn of(summary: &str) -> Result<Self, Never> {
        Ok(Notification { summary: summary.to_string(), body: None })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    Success,
    Failure(String),
}

fn worded(arguments: &[&str]) -> Result<Vec<String>, Never> {
    Ok(arguments.iter().map(|word| (*word).to_string()).collect())
}
