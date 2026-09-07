//! Everything a person reads on this screen, in the language they read.
//!
//! Two problems, one shape. The first is that the words were written in the
//! same voice as the comments around them -- *Stop before the battery does*,
//! *Nothing here answers to that* -- which is a good voice for somebody reading
//! the source and the wrong one for somebody holding the machine. The second is
//! that they were written in English, inline, in twenty-five crates, so there
//! was nowhere to put a second language even if somebody wrote one.
//!
//! ## How to say something
//!
//! A crate that has anything to say keeps a `words` module with one enum in it:
//! every sentence that crate can put on the screen, named for what it means
//! rather than for what it says. `say` turns one into words.
//!
//! ```ignore
//! use console_core_localization::{Said, say};
//!
//! pub enum Word { NightColoursOn, WarnWhenLow }
//!
//! impl Said for Word {
//!     fn english(&self) -> String {
//!         match self {
//!             Word::NightColoursOn => "Turn night colours on".to_string(),
//!             Word::WarnWhenLow => "Warn me when the battery gets low".to_string(),
//!         }
//!     }
//! }
//! ```
//!
//! One enum per crate rather than one for the desktop, because a single list of
//! every sentence on the machine is a file nobody can read and every crate has
//! to depend on. The crate that draws a thing is the crate that owns its words.
//!
//! ## What makes a second language possible
//!
//! `Tongue` and the `match` in `say`. Adding a language adds a variant there,
//! and then that `match` does not compile until `Said` has a method for it --
//! and `Said` does not compile until every enum in every crate has answered.
//! There is no way to add a language and quietly leave half the desktop in
//! English, and no way to add a sentence and quietly leave it untranslated.
//!
//! That is the whole mechanism. No catalogue to keep in step, no key that can
//! be misspelt into an empty string, no build step: the compiler is the thing
//! that says what is missing, which is the only checker anybody here has to
//! remember to run.
//!
//! ## The house style
//! Written for somebody who has never read a manual and is not going to. It is
//! a handheld console: the person holding it may be five, or eighty, or reading
//! their third language.  **Say what it does, not what it is.** A row is
//! something you press. *Turn night colours on*, not *Warm colours*. **Short.**
//! A row is one line on a small screen held at arm's length. If it does not
//! fit, the sentence is wrong, not the screen.  **Ordinary words.** Nothing a
//! person would not say out loud. No *dismiss*, no *configuration*, no
//! *authenticate*, no *unsupported*.  **Never clever.** *It has gone* and
//! *Nothing here answers to that* are writing. *Deleted* and *Nothing matched
//! that* are answers.  **Say what to do about it.** A message that reports a
//! problem and stops is a dead end. *There is no yt-dlp on this machine* tells
//! somebody a word they have never seen; *This needs a program the machine does
//! not have yet* tells them what happened.  **No jargon and no names of
//! programs**, unless the person chose that program themselves. `hyprsunset`,
//! `nmcli`, `powerprofilesctl` and `polkit` are this desktop's business, not
//! theirs.  `crates/console-core-localization/tests/the_house_style.rs` keeps
//! the parts of that a machine can check.

use std::sync::OnceLock;

use console_core_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tongue {
    English,
}

pub trait Said {
    fn english(&self) -> String;
}

pub fn say(what: &impl Said) -> Result<String, Never> {
    let Ok(tongue) = tongue();

    Ok(match tongue {
        Tongue::English => what.english(),
    })
}

pub fn tongue() -> Result<Tongue, Never> {
    static ASKED: OnceLock<Tongue> = OnceLock::new();

    Ok(*ASKED.get_or_init(|| {
        let Ok(asked) = asked();
        let Ok(read) = read(&asked);

        read
    }))
}

fn asked() -> Result<String, Never> {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let said = match std::env::var(name) {
            Ok(said) => said,
            Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => continue,
        };

        match said.trim().is_empty() {
            true => continue,
            false => return Ok(said),
        }
    }

    Ok(String::new())
}

pub fn read(locale: &str) -> Result<Tongue, Never> {
    let _language = locale.split(['_', '.', '@']).next().unwrap_or_default().to_lowercase();

    Ok(Tongue::English)
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Word {
        Hello,
    }

    impl Said for Word {
        fn english(&self) -> String {
            "Hello".to_string()
        }
    }

    #[test]
    fn something_said_comes_out_in_the_language_of_the_machine() {
        assert_eq!(say(&Word::Hello), Ok("Hello".to_string()));
    }

    #[test]
    fn a_language_nobody_has_written_reads_as_english() {
        for locale in ["", "C", "C.UTF-8", "nl_BE.UTF-8", "ja_JP", "rubbish"] {
            assert_eq!(read(locale), Ok(Tongue::English), "{locale}");
        }
    }

    #[test]
    fn the_country_and_the_encoding_are_not_part_of_the_answer() {
        assert_eq!(read("en_GB.UTF-8"), read("en_US"));
        assert_eq!(read("en"), read("en_IE.UTF-8@euro"));
    }
}
