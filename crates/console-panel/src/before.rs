//! What the machine said last time it was asked.
//!
//! `Page::meanwhile` is the tab as it can be drawn before the machine has
//! answered anything, and it works because most of a tab is known in advance:
//! the three power profiles are the same three whatever powerprofilesctl says,
//! and all the answer decides is which of them is marked.
//!
//! Some tabs are not like that. Sound is whatever is plugged in and whatever is
//! playing, Wi-Fi is whatever is in the air, Bluetooth is whatever has ever
//! been paired. There is nothing to draw in advance, so those tabs went up
//! empty and filled in, which is the whole card changing height a moment after
//! it appeared -- and it appeared under a thumb already moving down it.
//!
//! They are not unknowable, though. They were known last time, and last time is
//! a far better guess than nothing: the speakers are the speakers, and the
//! networks in this room are the networks that were in this room. So what a
//! command says is written down as it answers, and the tab's `meanwhile` builds
//! its rows out of what was written down.
//!
//! The same builder fed an older answer, never a second opinion about what the
//! tab looks like. A hand-written `meanwhile` is a second list somebody has to
//! remember to change when the first one changes; this one cannot drift,
//! because there is only one list and the two readings go into it.
//!
//! Under the cache and not beside the [`notes`](crate::notes), deliberately.
//! A note is something the desktop remembers about itself and could not work
//! out again -- which tab it was left on, how much room it was granted. This is
//! the machine's own answer to a question anybody can ask again, so it belongs
//! where a thing that can be rebuilt belongs. Somebody who clears the cache
//! gets a panel that opens the way it did before there was one, which is the
//! only thing any of this is allowed to cost.

use std::path::PathBuf;

use crate::running;

/// Where one answer is kept: named for the panel and for the question, so a
/// panel that learns to remember one more does not have to be taught where.
fn beside(note: &str) -> Option<PathBuf> {
    // A session with neither is one with nowhere of its own to keep an answer.
    // Nothing is remembered and every tab opens as it did the first time,
    // which is the same panel and only a slower one.
    let cache = match (std::env::var("XDG_CACHE_HOME"), std::env::var("HOME")) {
        (Ok(cache), _) => PathBuf::from(cache),
        (Err(_), Ok(home)) => PathBuf::from(home).join(".cache"),
        (Err(_), Err(_)) => return None,
    };

    Some(cache.join("console/asked").join(format!("{}.{}", whose(), filed(note))))
}

/// Which panel is asking, which is the program that is running.
fn whose() -> String {
    std::env::args()
        .next()
        .and_then(|argv0| {
            std::path::Path::new(&argv0).file_name().and_then(|name| name.to_str()).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "console-panel".to_string())
}

/// A question's name, as a file can be called.
///
/// Bluetooth asks one question per device and the name of a device is its
/// address, which is six numbers and five colons. A colon is a legal thing to
/// put in a filename here and an illegal thing to put in one nearly everywhere
/// else, so it does not go in one: anything that is not a letter, a number or a
/// dash becomes a dash. Two questions that differ only in punctuation would
/// collide, and none of them do -- the callers name their own questions, and
/// the names are in the source next to the commands.
fn filed(note: &str) -> String {
    let filed: String = note
        .chars()
        .map(|letter| match letter.is_ascii_alphanumeric() {
            true => letter.to_ascii_lowercase(),
            false => '-',
        })
        .collect();

    match filed.is_empty() {
        true => "asked".to_string(),
        false => filed,
    }
}

/// What it said the last time anybody asked, or nothing if nobody ever has.
///
/// Nothing is the right answer to never having asked, because it is what every
/// one of these readers already gets from a command that could not be run: an
/// empty reading is an empty list, which is the tab as it was before any of
/// this. Nothing here is allowed to be the difference between a panel that
/// draws and one that does not.
pub fn last(note: &str) -> String {
    let Some(path) = beside(note) else { return String::new() };

    let Ok(said) = std::fs::read_to_string(path) else { return String::new() };

    said
}

/// Run it, and write down what it said.
pub fn said(note: &str, argv: &[&str]) -> String {
    let said = running::said(argv);
    keep(note, &said);
    said
}

/// Write one down, if it is not already what is there.
///
/// The Sound tab is drawn again on every event pactl reports, which is several
/// a second while a volume is being turned, and all but the first of those say
/// what the file already says. Compared before it is written so that a thumb on
/// the rocker is not also a thumb on the disk.
fn keep(note: &str, said: &str) {
    match last(note) == said {
        true => return,
        false => {},
    }

    let Some(path) = beside(note) else { return };

    let Some(holding) = path.parent() else { return };

    match std::fs::create_dir_all(holding) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console: {}: keeping what a tab last said: {fault}", holding.display());

            return;
        }
    }

    let _ = std::fs::write(path, said);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_is_filed_under_a_name_any_filesystem_would_take() {
        assert_eq!(filed("sinks"), "sinks");
        assert_eq!(filed("bluetooth AA:BB:CC:DD:EE:FF"), "bluetooth-aa-bb-cc-dd-ee-ff");
        assert_eq!(filed("opens audio/x-opus+ogg"), "opens-audio-x-opus-ogg");
    }

    /// A question with no name at all would otherwise be filed as the panel and
    /// a trailing dot, which is a hidden file nobody meant to make.
    #[test]
    fn a_question_with_no_name_is_still_a_file() {
        assert_eq!(filed(""), "asked");
        assert_eq!(filed("///"), "---");
    }

    /// The whole of what this is for: a question nobody has ever asked answers
    /// the way a command that could not be run answers, and the tab drawn from
    /// it is the tab as it was before any of this.
    ///
    /// The cache is left where it is rather than pointed somewhere else. Two
    /// tests setting an environment variable while the rest of the suite runs
    /// beside them is a race for the sake of a file that is not there either
    /// way, and a question named this is a question nothing has ever asked.
    #[test]
    fn a_question_nobody_has_asked_says_nothing() {
        assert_eq!(last("a question nothing on this machine has ever asked"), "");
    }
}
