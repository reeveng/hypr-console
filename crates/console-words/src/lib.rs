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
//! use console_words::{Said, say};
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
//!
//! Written for somebody who has never read a manual and is not going to. It is
//! a handheld console: the person holding it may be five, or eighty, or reading
//! their third language.
//!
//! **Say what it does, not what it is.** A row is something you press. *Turn
//! night colours on*, not *Warm colours*.
//!
//! **Short.** A row is one line on a small screen held at arm's length. If it
//! does not fit, the sentence is wrong, not the screen.
//!
//! **Ordinary words.** Nothing a person would not say out loud. No *dismiss*,
//! no *configuration*, no *authenticate*, no *unsupported*.
//!
//! **Never clever.** *It has gone* and *Nothing here answers to that* are
//! writing. *Deleted* and *Nothing matched that* are answers.
//!
//! **Say what to do about it.** A message that reports a problem and stops is a
//! dead end. *There is no yt-dlp on this machine* tells somebody a word they
//! have never seen; *This needs a program the machine does not have yet* tells
//! them what happened.
//!
//! **No jargon and no names of programs**, unless the person chose that program
//! themselves. `hyprsunset`, `nmcli`, `powerprofilesctl` and `polkit` are this
//! desktop's business, not theirs.
//!
//! `crates/console-words/tests/the_house_style.rs` keeps the parts of that a
//! machine can check.

use std::sync::OnceLock;

/// A language the desktop can be read in.
///
/// One, for now. The point of the enum is that the second one cannot be added
/// halfway: every `match` on it has to answer, and there is one in `say`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tongue {
    English,
}

/// Anything the desktop can say, in each language it can say it in.
///
/// One method per language, and no default: a language with a default would be
/// a language that silently falls back to English on every sentence somebody
/// forgot, which is the failure this whole arrangement exists to make
/// impossible.
pub trait Said {
    fn english(&self) -> String;
}

/// One thing, in the language this machine is being read in.
pub fn say(what: &impl Said) -> String {
    match tongue() {
        Tongue::English => what.english(),
    }
}

/// The language this machine is being read in.
///
/// Asked once. It comes from the environment the session was started with and
/// nothing changes it while the desktop is up, so a panel that read it per row
/// would be asking the same question of `getenv` a hundred times to be told the
/// same thing.
pub fn tongue() -> Tongue {
    static ASKED: OnceLock<Tongue> = OnceLock::new();
    *ASKED.get_or_init(|| read(&asked()))
}

/// What the session says it is, in the order the C library reads it.
fn asked() -> String {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(said) = std::env::var(name) {
            if !said.trim().is_empty() {
                return said;
            }
        }
    }

    String::new()
}

/// Which language a locale is, or English where it is one nobody has written.
///
/// The country and the encoding are cut off first: `nl_BE.UTF-8` and `nl_NL`
/// are one language to this desktop, and they would be two if the whole string
/// were matched. A locale nobody here has words for reads as English rather
/// than as nothing, because a screen in a language you do not read is still a
/// screen you can use and an empty one is not.
pub fn read(locale: &str) -> Tongue {
    let _language = locale.split(['_', '.', '@']).next().unwrap_or_default().to_lowercase();
    Tongue::English
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
        assert_eq!(say(&Word::Hello), "Hello");
    }

    /// A machine set to something nobody has written words for is still a
    /// machine somebody is holding.
    #[test]
    fn a_language_nobody_has_written_reads_as_english() {
        for locale in ["", "C", "C.UTF-8", "nl_BE.UTF-8", "ja_JP", "rubbish"] {
            assert_eq!(read(locale), Tongue::English, "{locale}");
        }
    }

    /// The country and the encoding are not the language. Written down now
    /// because the first language added is the moment it starts to matter, and
    /// by then this is easy to get wrong in a way that only shows up in
    /// Belgium.
    #[test]
    fn the_country_and_the_encoding_are_not_part_of_the_answer() {
        assert_eq!(read("en_GB.UTF-8"), read("en_US"));
        assert_eq!(read("en"), read("en_IE.UTF-8@euro"));
    }
}
