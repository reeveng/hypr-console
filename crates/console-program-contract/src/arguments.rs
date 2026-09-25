//! The command line, read once and handed in.
//!
//! A program's `main` is the only part of it allowed to ask the process what it
//! was started with, and everything after that is handed this. It matters more
//! than it looks: `console-wallpaper --now` and `console-wallpaper` are two
//! behaviors of one program, and today that fork is a call to `std::env::args` in
//! the middle of the deciding, where no test can put a different answer.

use console_core_never::Never;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arguments {
    words: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Present,
    Absent,
}

impl Arguments {
    pub fn of(words: &[&str]) -> Result<Self, Never> {
        Ok(Arguments { words: words.iter().map(|word| (*word).to_string()).collect() })
    }

    pub fn words(&self) -> Result<&[String], Never> {
        Ok(&self.words)
    }

    pub fn first(&self) -> Result<Option<&str>, Never> {
        Ok(self.words.first().map(String::as_str))
    }

    pub fn given(&self, word: &str) -> Result<Flag, Never> {
        Ok(match self.words.iter().any(|given| given == word) {
            true => Flag::Present,
            false => Flag::Absent,
        })
    }

    pub fn after(&self, word: &str) -> Result<Option<&str>, Never> {
        let next = self.words.iter().skip_while(|given| *given != word).nth(1);

        Ok(next.map(String::as_str))
    }
}
