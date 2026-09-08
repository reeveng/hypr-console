//! The command line, read once and handed in.
//!
//! A program's `main` is the only part of it allowed to ask the process what
//! it was started with, and everything after that is handed this. It matters
//! more than it looks: `console-sky --now` and `console-sky` are two
//! behaviours of one program, and today that fork is a call to
//! `std::env::args` in the middle of the deciding, where no test can put a
//! different answer.

use console_core_never::Never;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Argv {
    words: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Given {
    Yes,
    No,
}

impl Argv {
    pub fn of(words: &[&str]) -> Result<Self, Never> {
        Ok(Argv { words: words.iter().map(|word| (*word).to_string()).collect() })
    }

    pub fn words(&self) -> Result<&[String], Never> {
        Ok(&self.words)
    }

    pub fn first(&self) -> Result<Option<&str>, Never> {
        Ok(self.words.first().map(String::as_str))
    }

    pub fn given(&self, word: &str) -> Result<Given, Never> {
        Ok(match self.words.iter().any(|given| given == word) {
            true => Given::Yes,
            false => Given::No,
        })
    }

    pub fn after(&self, word: &str) -> Result<Option<&str>, Never> {
        let at = match self.words.iter().position(|given| given == word) {
            Some(at) => at,
            None => return Ok(None),
        };

        let next = self.words.iter().skip(at).nth(1);

        Ok(next.map(String::as_str))
    }
}
