//! What the panel tells `console-machine` to set, in the one word both use.
//!
//! The rows that choose a language, an hour and a name run the program, and
//! the program reads the same word back off its command line. With the word
//! in each of them, the two readings stand one typo apart from a row that runs
//! a word the program refuses, and the refusal goes to a journal nobody
//! holding the device reads.

use console_core_arguments::Subcommand;
use console_core_never::Never;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum MachineSetting {
    #[words(word = "language", about = "make the language NAME CHARSET unless the machine has it, then use it")]
    Language,
    #[words(word = "hour", about = "keep the hour in ZONE")]
    Hour,
    #[words(word = "name", about = "answer to NAME on a network")]
    Name,
}

impl Subcommand for MachineSetting {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(MachineSetting::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        MachineSetting::about(self)
    }
}
