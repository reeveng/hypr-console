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

/// One language the paddle can be told to listen for.
pub struct Language {
    /// What whisper is given, which is its own two letters -- or `auto`, which
    /// is whisper's word for working it out.
    pub key: &'static str,
    /// What it is called on the row somebody chooses it from.
    pub says: &'static str,
}

/// The languages offered, in the order they are drawn.
///
/// The three this device is spoken to in, under the machine deciding for
/// itself. Chinese was a fourth for as long as it took to notice that nobody
/// was dictating any.
///
/// This list is not what whisper can hear. The model has ninety-nine languages
/// in it and `auto` reaches all of them; this is the short list of the ones
/// worth a row, and a fourth language spoken here would be a fourth row rather
/// than a different model.
pub const EVERY: [Language; 4] = [
    Language { key: "auto", says: "Whichever is spoken" },
    Language { key: "en", says: "English" },
    Language { key: "nl", says: "Dutch" },
    Language { key: "th", says: "Thai" },
];

/// What is listened for when nothing has been chosen.
pub const UNLESS_TOLD: &str = "auto";

/// What the choice is called in the file it is written to.
const SETTING: &str = "dictation";

/// The language the paddle is listening for.
///
/// Read on every press rather than held anywhere, because a press is the next
/// thing that happens after the panel is closed and there is nothing between
/// them to tell.
///
/// A key written into the file by hand and spelled wrong is `auto` rather than
/// an error: whisper answers an unknown language by refusing the recording,
/// which would be a paddle that has silently stopped working.
pub fn chosen() -> String {
    let said = console_defaults::setting(SETTING).unwrap_or_default();
    match one(&said).is_some() {
        true => said,
        false => UNLESS_TOLD.to_string(),
    }
}

/// One language by its key.
pub fn one(key: &str) -> Option<&'static Language> {
    EVERY.iter().find(|language| language.key == key)
}

/// Listen for this one from now on.
pub fn choose(key: &str) {
    console_defaults::set(SETTING, key);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key spelled wrong is whisper refusing the recording, which is a
    /// paddle that has stopped working and says nothing about why.
    #[test]
    fn the_one_used_when_nothing_has_been_chosen_is_one_of_them() {
        assert!(one(UNLESS_TOLD).is_some());
        assert!(one("elvish").is_none());
    }

    /// Whisper is given a two-letter language or its own word for guessing,
    /// and anything else is a recording it will not read.
    #[test]
    fn every_language_is_named_the_way_the_hearing_names_it() {
        for language in &EVERY {
            let spelled = language.key == UNLESS_TOLD || language.key.len() == 2;
            assert!(spelled, "{} is not a language whisper takes", language.key);
            assert!(language.key.chars().all(|one| one.is_ascii_lowercase()));
        }
    }

    /// Working it out is a choice like the others, and it is the first row
    /// because it is what the paddle does until somebody says otherwise.
    #[test]
    fn deciding_for_itself_is_offered_and_offered_first() {
        assert_eq!(EVERY[0].key, UNLESS_TOLD);
    }

    #[test]
    fn every_language_is_named_once() {
        let mut keys: Vec<&str> = EVERY.iter().map(|language| language.key).collect();
        let named = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), named, "a language is named twice");
    }

    /// The one that was taken out. Nobody dictates it, and a row nobody
    /// presses is a row between somebody and the row they wanted.
    #[test]
    fn chinese_is_not_offered() {
        assert!(one("zh").is_none());
    }
}
