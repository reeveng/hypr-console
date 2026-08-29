//! Speaking instead of typing.
//!
//! A handheld has no keyboard, and the one it draws on the screen is a thumb
//! hunting for letters over half the picture. So the bottom left paddle takes
//! what is said and writes it into whatever holds the focus, which is the same
//! thing a keyboard does and none of the walking.
//!
//! Three programs already know how to do the parts of this. `pw-record` takes
//! the microphone, `whisper-cli` turns a recording into words, and `wtype`
//! writes them where a keyboard would have. What is here is the deciding: the
//! shape of each of those calls, where the recording waits between the two
//! presses, and what to do with what comes back.
//!
//! Nothing in this file touches a device or starts a program, so what would be
//! run can be asked for and looked at without a microphone in the room.

use std::path::{Path, PathBuf};

/// The words model, and where it is kept.
///
/// Turbo rather than the small models, because what is dictated here is four
/// languages and the small ones are only good at one of them. It is quantised
/// because the difference on this machine is half a gigabyte of disk and a
/// difference in the words nobody has been able to hear.
pub const MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

/// Where the model is fetched from, which is the whisper.cpp project's own.
pub const MODEL_FROM: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";

/// What a recording is, in the shape whisper wants it: one channel of sixteen
/// thousand samples a second. Anything else is resampled before it is heard,
/// so recording it right costs nothing and saves that.
pub const RATE: &str = "16000";

/// How many cores the hearing is given.
///
/// Fewer than the machine has. The press that stops the recording is one a
/// thumb is waiting on, and a machine with every core spoken for is one where
/// the compositor cannot draw the notification saying what was heard.
pub const THREADS: &str = "8";

/// Where a thing lives that is gone at logout.
fn runtime() -> PathBuf {
    let said = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    let at = match said.is_empty() {
        true => "/tmp".to_string(),
        false => said,
    };
    Path::new(&at).join("console").join("voice")
}

/// Where a thing lives that outlives a session.
fn kept() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let share = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|said| !said.is_empty())
        .unwrap_or_else(|| format!("{home}/.local/share"));
    Path::new(&share).join("console").join("voice")
}

/// The recording, between the press that starts it and the press that ends it.
pub fn said() -> PathBuf {
    runtime().join("said.wav")
}

/// What says a recording is going on, and which program is doing it.
///
/// The file is the whole of the state: one press finds it and stops, and one
/// press does not and starts. A session that dies mid-sentence leaves it
/// behind, and the runtime directory is emptied at logout, which is why it
/// lives there rather than beside the model.
pub fn taking() -> PathBuf {
    runtime().join("taking.pid")
}

/// The model, wherever this machine keeps it.
pub fn model() -> PathBuf {
    kept().join(MODEL)
}

/// Take the microphone, and write it down.
pub fn recording(into: &Path) -> Vec<String> {
    [
        "pw-record",
        "--rate",
        RATE,
        "--channels",
        "1",
        "--format",
        "s16",
        &into.to_string_lossy(),
    ]
    .map(str::to_string)
    .to_vec()
}

/// Turn a recording into words.
///
/// The language is asked for rather than said, because what is dictated into
/// this is four of them and which one is a thing the recording knows and the
/// button does not.
pub fn hearing(model: &Path, said: &Path) -> Vec<String> {
    [
        "whisper-cli",
        "--model",
        &model.to_string_lossy(),
        "--file",
        &said.to_string_lossy(),
        "--language",
        "auto",
        "--threads",
        THREADS,
        "--no-timestamps",
        "--no-prints",
    ]
    .map(str::to_string)
    .to_vec()
}

/// Write words where a keyboard would have.
pub fn typing(words: &str) -> Vec<String> {
    ["wtype", "--", words].map(str::to_string).to_vec()
}

/// Fetch the model, once.
pub fn fetching(into: &Path) -> Vec<String> {
    ["curl", "--location", "--fail", "--silent", "--show-error", "--output", &into.to_string_lossy(), MODEL_FROM]
        .map(str::to_string)
        .to_vec()
}

/// What was heard, as a person would have typed it.
///
/// Whisper writes a line per stretch of speech and marks what is not speech at
/// all, so a room with nobody talking in it comes back as `[BLANK_AUDIO]`
/// rather than as nothing. Those marks are what somebody would have to delete
/// by hand, on a device with no keyboard, which is the thing this exists to
/// avoid.
pub fn tidy(heard: &str) -> String {
    let words: Vec<&str> = heard
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_a_noise(line))
        .collect();
    words.join(" ").split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Whether a line is whisper describing the room rather than quoting it.
fn is_a_noise(line: &str) -> bool {
    let bracketed = |open: char, close: char| line.starts_with(open) && line.ends_with(close);
    bracketed('[', ']') || bracketed('(', ')') || bracketed('*', '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_is_one_channel_at_the_rate_the_hearing_wants() {
        let argv = recording(Path::new("/run/said.wav"));
        assert_eq!(argv.first().map(String::as_str), Some("pw-record"));
        assert!(argv.windows(2).any(|pair| pair == ["--rate", RATE]));
        assert!(argv.windows(2).any(|pair| pair == ["--channels", "1"]));
        assert_eq!(argv.last().map(String::as_str), Some("/run/said.wav"));
    }

    #[test]
    fn the_hearing_is_told_the_model_the_file_and_to_say_nothing_else() {
        let argv = hearing(Path::new("/keep/model.bin"), Path::new("/run/said.wav"));
        assert!(argv.windows(2).any(|pair| pair == ["--model", "/keep/model.bin"]));
        assert!(argv.windows(2).any(|pair| pair == ["--file", "/run/said.wav"]));
        assert!(argv.iter().any(|word| word == "--no-timestamps"));
        assert!(argv.iter().any(|word| word == "--no-prints"));
    }

    /// Four languages are dictated into this and the button cannot know which.
    #[test]
    fn the_language_is_asked_of_the_recording_rather_than_assumed() {
        let argv = hearing(Path::new("m"), Path::new("s"));
        assert!(argv.windows(2).any(|pair| pair == ["--language", "auto"]));
    }

    /// A double dash, because what was said is a sentence and a sentence can
    /// start with a word wtype would read as a flag.
    #[test]
    fn what_is_typed_is_handed_over_as_words_and_not_as_options() {
        assert_eq!(typing("-n is a flag"), ["wtype", "--", "-n is a flag"]);
    }

    #[test]
    fn what_was_heard_comes_back_as_one_line() {
        assert_eq!(tidy("  Hello there.\n\n  How are you?  \n"), "Hello there. How are you?");
    }

    /// A room with nobody talking in it is nothing said, not a word to type.
    #[test]
    fn the_marks_for_what_is_not_speech_are_not_words() {
        assert_eq!(tidy("[BLANK_AUDIO]"), "");
        assert_eq!(tidy("(wind)\nHello\n*laughs*"), "Hello");
    }

    #[test]
    fn the_recording_waits_where_a_session_ending_clears_it() {
        // SAFETY: one thread, and both variables are put back before it ends.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        unsafe { std::env::set_var("XDG_DATA_HOME", "/home/somebody/.local/share") };
        assert_eq!(said(), Path::new("/run/user/1000/console/voice/said.wav"));
        assert_eq!(taking(), Path::new("/run/user/1000/console/voice/taking.pid"));
        assert_eq!(model(), Path::new("/home/somebody/.local/share/console/voice").join(MODEL));
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }
}
