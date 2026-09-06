//! Which language the paddle is listening for.
//!
//! It used to be asked of the recording every time, on the grounds that which
//! language is being spoken is a thing the recording knows and a button does
//! not. That is true of a sentence and false of a word.
//!
//! Detection is a guess made on what was said, and most of what is said to
//! this paddle is one or two words into a search box. There is not enough of
//! them to guess from, and what whisper guesses when there is not enough is
//! English -- so a Dutch word comes back as the English word it sounds nearest
//! to, and a Thai one comes back as English letters spelling the sound of it.
//! Which is the case the button is pressed in most, failing in the way that is
//! hardest to see: it is a word, it is spelled correctly, and it is the wrong
//! word.
//!
//! So it can be told instead. Somebody writing Dutch all afternoon says so
//! once, and every press that afternoon is read as Dutch. It is also half the
//! wait: detection is a whole extra pass of the encoder, measured on this
//! device at 2.7 seconds asked against 1.4 told.
//!
//! Asking is still what it does until it is told otherwise. There is no
//! language to default to that is not wrong for two of the three, and a guess
//! that is sometimes wrong is better than a setting that is always wrong for
//! somebody.

use console_never::Never;

pub struct Language {
    pub key: &'static str,
    pub says: &'static str,
}

pub const EVERY: [Language; 4] = [
    Language { key: "auto", says: "Whichever is spoken" },
    Language { key: "en", says: "English" },
    Language { key: "nl", says: "Dutch" },
    Language { key: "th", says: "Thai" },
];

pub const UNLESS_TOLD: &str = "auto";

const SETTING: &str = "dictation";

pub fn chosen() -> Result<String, Never> {
    let told = console_defaults::setting(SETTING)?;
    let said = told.unwrap_or_default();
    let known = one(&said)?;

    Ok(match known.is_some() {
        true => said,
        false => UNLESS_TOLD.to_string(),
    })
}

pub fn one(key: &str) -> Result<Option<&'static Language>, Never> {
    Ok(EVERY.iter().find(|language| language.key == key))
}

pub fn choose(key: &str) -> Result<(), Never> {
    console_defaults::set(SETTING, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_one_used_when_nothing_has_been_chosen_is_one_of_them() {
        assert_eq!(one(UNLESS_TOLD).map(|found| found.is_some()), Ok(true));
        assert_eq!(one("elvish").map(|found| found.is_none()), Ok(true));
    }

    #[test]
    fn every_language_is_named_the_way_the_hearing_names_it() {
        for language in &EVERY {
            let spelled = language.key == UNLESS_TOLD || language.key.len() == 2;
            assert!(spelled, "{} is not a language whisper takes", language.key);
            assert!(language.key.chars().all(|one| one.is_ascii_lowercase()));
        }
    }

    #[test]
    fn deciding_for_itself_is_offered_and_offered_first() {
        assert_eq!(EVERY[0].key, UNLESS_TOLD);
    }

    #[test]
    fn every_language_is_named_once() {
        let mut seen = BTreeSet::new();
        let twice: Vec<&str> =
            EVERY.iter().map(|language| language.key).filter(|key| !seen.insert(*key)).collect();

        assert!(twice.is_empty(), "{twice:?} is named twice");
    }

    #[test]
    fn chinese_is_not_offered() {
        assert_eq!(one("zh").map(|found| found.is_none()), Ok(true));
    }
}
