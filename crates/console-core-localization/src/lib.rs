//! Everything a person reads on this screen, in the language they read.
//!
//! Two problems, one shape. The first is that the words were written in the
//! same voice as the comments around them -- *Stop before the battery does*,
//! *Nothing here answers to that* -- which is a good voice for someone reading
//! the source and the wrong one for someone holding the machine. The second is
//! that they were written in English, inline, in twenty-five crates, so there
//! was nowhere to put a second language even if someone wrote one.
//!
//! ## How to put words on the screen
//!
//! A crate that has anything to say keeps a `words` module with one enum in it:
//! every sentence that crate can put on the screen, named for what it means
//! rather than for what it says. `text` turns one into words.
//!
//! ```ignore
//! use console_core_localization::{Localized, text};
//!
//! pub enum Word { NightColors, WarnWhenLow }
//!
//! impl Localized for Word {
//!     fn english(&self) -> String {
//!         match self {
//!             Word::NightColors => "Night colors".to_string(),
//!             Word::WarnWhenLow => "Warn me when the battery gets low".to_string(),
//!         }
//!     }
//! }
//! ```
//!
//! One enum per crate rather than one for the desktop, because a single list of
//! every sentence on the machine is a file no one can read and every crate has
//! to depend on. The crate that draws a thing is the crate that owns its words.
//!
//! ## What makes a second language possible
//!
//! `Language` and the `match` in `text`. Adding a language adds a variant there,
//! and then that `match` does not compile until `Localized` has a method for it --
//! and `Localized` does not compile until every enum in every crate has answered.
//! There is no way to add a language and quietly leave half the desktop in
//! English, and no way to add a sentence and quietly leave it untranslated.
//!
//! That is the whole mechanism. No catalog to keep in step, no key that can
//! be misspelt into an empty string, no build step: the compiler is the thing
//! that says what is missing, which is the only checker anyone here has to
//! remember to run.
//!
//! ## Asked again every time
//!
//! What language someone reads was once read once and kept for the life of the
//! process, which is what a memo is and is the reason it went: the first
//! caller's answer becomes every later caller's, and a check meaning to press
//! the other language has nowhere to stand. Three environment reads cost less
//! than a row costs to draw, so the question is asked again at every `text`.
//!
//! ## The house style
//! Written for someone who has never read a manual and is not going to. It is
//! a handheld console: the person holding it may be five, or eighty, or reading
//! their third language.  **A row that does something says what it does.**
//! *Clear all*, *Forget*, *Convert* -- the verb, and what it happens to if the
//! row does not already sit under it.  **A row that shows a setting names the
//! setting, and its state stands beside it.** *Wi-Fi* with *Off* to the right,
//! never *Turn Wi-Fi on*: the label is what a person looks for and it has to
//! stay the same when they change the thing, or the row they just pressed is
//! not the row they are looking at. Everything worth copying is written that
//! way, and so is every switch here.  **Short.** A row is one line on a small
//! screen held at arm's length. If it does not fit, the sentence is wrong, not
//! the screen. A word for going ahead is one or two, and no longer.  **One act,
//! one word.** The menu row, the question it raises and the answer under it say
//! the same verb. *Delete* under *Throw this away?* was three surfaces and two
//! vocabularies for one press.  **Nothing stands for the thing already on the
//! screen.** A card titled with a device does not offer *Pair with Blue Keys*,
//! and a question that names what it is about does not answer *Put them back*.
//! **Ordinary words.** Nothing a person would not say out loud. No *dismiss*,
//! no *configuration*, no *authenticate*, no *unsupported*.  **Never clever.**
//! *It has gone* and *Nothing here answers to that* are writing. *Deleted* and
//! *Nothing matched that* are answers.  **Say what to do about it.** A message
//! that reports a problem and stops is a dead end. *There is no yt-dlp on this
//! machine* tells someone a word they have never seen; *This needs a program
//! the machine does not have yet* tells them what happened.  **No jargon and no
//! names of programs**, unless the person chose that program themselves.
//! `hyprsunset`, `nmcli`, `powerprofilesctl` and `polkit` are this desktop's
//! business, not theirs.  The part of that a machine can check is a test in
//! each crate that has words: `every_word_fits_a_row_and_names_no_program`,
//! `a_switch_names_the_thing_and_its_two_states_are_told_apart`, and
//! `the_word_for_going_ahead_is_short_and_stands_for_nothing`.

use std::time::Duration;

use console_core_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
}

pub trait Localized {
    fn english(&self) -> String;
}

pub fn text(what: &impl Localized) -> Result<String, Never> {
    let Ok(language) = language();

    Ok(match language {
        Language::English => what.english(),
    })
}

pub fn language() -> Result<Language, Never> {
    let Ok(asked) = asked();

    read(&asked)
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "the three names the standard gives for what language someone reads, in the crate that is what reading them means"
    )
)]
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

pub fn read(_locale: &str) -> Result<Language, Never> {
    Ok(Language::English)
}

pub fn positional(elapsed: Duration) -> Result<String, Never> {
    let whole = elapsed.as_secs();
    let (hours, minutes, seconds) = (
        whole.saturating_div(3600),
        whole.saturating_div(60).wrapping_rem(60),
        whole.wrapping_rem(60),
    );

    Ok(match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Word {
        Hello,
    }

    impl Localized for Word {
        fn english(&self) -> String {
            "Hello".to_string()
        }
    }

    #[test]
    fn words_come_out_in_the_language_of_the_machine() {
        assert_eq!(text(&Word::Hello), Ok("Hello".to_string()));
    }

    #[test]
    fn a_language_no_one_has_written_reads_as_english() {
        for locale in ["", "C", "C.UTF-8", "nl_BE.UTF-8", "ja_JP", "rubbish"] {
            assert_eq!(read(locale), Ok(Language::English), "{locale}");
        }
    }

    #[test]
    fn a_length_is_said_with_hours_only_where_there_are_hours() {
        for (seconds, said) in [(0, "0:00"), (9, "0:09"), (249, "4:09"), (3600, "1:00:00"), (3849, "1:04:09")] {
            assert_eq!(positional(Duration::from_secs(seconds)), Ok(said.to_string()), "{seconds}");
        }
    }

    #[test]
    fn a_part_of_a_second_is_not_a_second() {
        assert_eq!(positional(Duration::from_millis(59_999)), Ok("0:59".to_string()));
    }

    #[test]
    fn the_country_and_the_encoding_are_not_part_of_the_answer() {
        assert_eq!(read("en_GB.UTF-8"), read("en_US"));
        assert_eq!(read("en"), read("en_IE.UTF-8@euro"));
    }
}
