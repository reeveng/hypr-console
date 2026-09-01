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

pub mod languages;

use std::path::{Path, PathBuf};

/// The words model, and where it is kept.
///
/// Large-v3-turbo, because what is dictated into this is English, Dutch and
/// Thai, and the small models are good at one of those.
///
/// It was small for a few hours, and the reason is worth keeping. On the
/// processor this model spent 19.4 seconds reading a two-second sentence,
/// which is not a slow dictation but no dictation: the box the words were
/// meant for is closed by then and they are typed into whatever the desktop is
/// showing instead. Small did it in 4.5 and was chosen for that alone.
///
/// Then the hearing was pointed at the machine's own graphics -- see
/// [`whisper`] -- and the same model came back in 2.7 seconds, quicker than
/// small had ever been on the processor. So the trade that was made under
/// protest was handed back: this is the accurate one, and it is also the fast
/// one now.
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
/// Fewer than the machine's sixteen. The press that stops the recording is one
/// a thumb is waiting on, and a machine with every core spoken for is one where
/// the compositor cannot draw the notification saying what was heard.
///
/// Twelve rather than eight because it was measured, and rather than sixteen
/// for the same reason: on the processor the same sentence took 5.0 seconds on
/// eight, 3.6 on twelve and 4.6 on sixteen. Past twelve the cores queue for
/// memory instead of working, and the four left over are what draws the screen.
///
/// It matters much less now that the encoder runs on the graphics card, and it
/// is left where the measuring put it because the processor is still what
/// answers the first press on a machine that has not built the other one yet.
pub const THREADS: &str = "12";

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
///
/// Named after the press that made it, rather than one name shared by all of
/// them. Every recording used to be `said.wav`, and two presses close together
/// fought over it: the press that stops a recording goes on to read it, which
/// takes as long as the hearing takes, and a press arriving inside that time
/// started a new recording into the same name and then had it deleted
/// underneath by the first press finishing its tidy-up.
///
/// What that leaves is a recorder writing to a file with no name left in the
/// directory, and a stop press handing whisper a path to nothing: exit 2, "What
/// was said could not be read", and a paddle that has eaten the sentence. It is
/// not rare, either. It is what pressing the button again because it seemed not
/// to work does every time.
pub fn said(press: u32) -> PathBuf {
    runtime().join(format!("said-{press}.wav"))
}

/// What says a recording is going on: who holds the microphone, and what it is
/// filling.
///
/// The file is the whole of the state: one press finds it and stops, and one
/// press does not and starts. A session that dies mid-sentence leaves it
/// behind, and the runtime directory is emptied at logout, which is why it
/// lives there rather than beside the model.
pub fn taking() -> PathBuf {
    runtime().join("taking.pid")
}

/// The note a press leaves for the press after it.
///
/// Two numbers: the recorder to stop, and the recording to read when it has
/// stopped. The second is there because the first is not enough to find the
/// file any more, now that each press has one of its own.
pub fn taken(recorder: u32, press: u32) -> String {
    format!("{recorder} {press}")
}

/// The same note, read back, where it says anything this understands.
///
/// A note from an older build holds one number and names no recording. It is
/// refused rather than guessed at: the recording it meant is `said.wav`, which
/// nothing writes any more, so a press that believed it would stop a recorder
/// and then hear silence.
pub fn told_by(note: &str) -> Option<(i32, u32)> {
    let mut words = note.split_whitespace();
    let recorder = words.next()?.parse().ok()?;
    let press = words.next()?.parse().ok()?;
    Some((recorder, press))
}

/// The model, wherever this machine keeps it.
pub fn model() -> PathBuf {
    kept().join(MODEL)
}

/// Where whisper.cpp is taken from, and which of it.
///
/// Pinned, because this is a compiler being pointed at somebody else's
/// repository on a machine somebody is holding. A tag is a thing that was
/// tested here; a branch is whatever it happens to be on the morning the
/// device is rebuilt.
pub const WHISPER_FROM: &str = "https://github.com/ggml-org/whisper.cpp";

/// The tag this desktop has run.
pub const WHISPER_AT: &str = "v1.9.1";

/// The hearing this machine built for itself, if it has.
///
/// Beside the model, under the same rule and for the same reason: it is not
/// source and it does not belong in the repository, and a device rebuilt from
/// the manifest makes it once rather than carrying it.
pub fn whisper() -> PathBuf {
    kept().join("whisper-cli")
}

/// Where it is made, which is not where it is kept.
///
/// A build tree is two gigabytes of object files and a working copy of
/// somebody else's project, and none of it outlives the build. So it is made
/// somewhere a session ending clears, and only the one file that matters is
/// carried out.
pub fn making() -> PathBuf {
    runtime().join("whisper.cpp")
}

/// Fetch the source of the hearing, at the tag this desktop has run.
pub fn cloning(into: &Path) -> Vec<String> {
    [
        "git",
        "clone",
        "--quiet",
        "--depth",
        "1",
        "--branch",
        WHISPER_AT,
        WHISPER_FROM,
        &into.to_string_lossy(),
    ]
    .map(str::to_string)
    .to_vec()
}

/// Point it at the graphics card, and at nothing it does not need.
///
/// Vulkan because that is what this machine's own drivers speak, and because
/// the packaged build speaks nothing but the processor: `system_info` reports
/// no backend at all beside the CPU, on a handheld whose iGPU is built for
/// exactly the arithmetic the encoder is made of.
///
/// Static, because what comes out of this is one file installed under one
/// name, beside a model, on a machine that already has a different libwhisper
/// in `/usr/lib`. A build that needed its own libraries found first would be a
/// build that quietly decides which of the two is running.
pub fn configuring(at: &Path) -> Vec<String> {
    [
        "cmake",
        "-S",
        &at.to_string_lossy(),
        "-B",
        &at.join("build").to_string_lossy(),
        "-DGGML_VULKAN=ON",
        "-DBUILD_SHARED_LIBS=OFF",
        "-DCMAKE_BUILD_TYPE=Release",
        "-DWHISPER_BUILD_TESTS=OFF",
    ]
    .map(str::to_string)
    .to_vec()
}

/// Build the one program out of it that this desktop runs.
pub fn compiling(at: &Path) -> Vec<String> {
    [
        "cmake",
        "--build",
        &at.join("build").to_string_lossy(),
        "--target",
        "whisper-cli",
        "--parallel",
    ]
    .map(str::to_string)
    .to_vec()
}

/// Where the build leaves it.
pub fn made(at: &Path) -> PathBuf {
    at.join("build").join("bin").join("whisper-cli")
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
/// The language is whatever was chosen, which is `auto` until somebody
/// chooses. Asking the recording is the right question about a sentence and
/// the wrong one about a word: there is not enough of one or two words to
/// guess a language from, and what whisper guesses without enough is English.
/// See [`languages`].
pub fn hearing(whisper: &Path, model: &Path, said: &Path, language: &str) -> Vec<String> {
    [
        &whisper.to_string_lossy(),
        "--model",
        &model.to_string_lossy(),
        "--file",
        &said.to_string_lossy(),
        "--language",
        language,
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

/// How long a piece of a recording is looked at, in samples.
///
/// Twenty milliseconds at the rate the recording is made. Long enough to have
/// a level worth measuring and short enough that a gap between two words is
/// several of them.
const FRAME: usize = 320;

/// What a recording sounds like: the quiet of it, and the loud of it.
///
/// Both are frames rather than samples. A single sample is a click, and a
/// click is the one thing a room is full of.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    /// The middle frame, which in a recording of a room is the room.
    pub middle: f32,
    /// The frame nine tenths of the way up, which in a recording of somebody
    /// talking is somebody talking.
    pub loud: f32,
}

/// What a recording sounds like.
///
/// The recording is what `pw-record` was told to make: one channel of signed
/// sixteen-bit samples. The header is walked rather than assumed to be
/// forty-four bytes, because a wav is chunks and a writer is allowed to put
/// its own among them.
///
/// Nothing is opened here. The bytes are handed in, so a recording of a room
/// can be measured in a test without a room.
pub fn level(wav: &[u8]) -> Level {
    let Some(sound) = data(wav) else { return Level::default() };
    let mut frames: Vec<f32> = sound
        .chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32768.0)
        .collect::<Vec<f32>>()
        .chunks_exact(FRAME)
        .map(|frame| (frame.iter().map(|one| one * one).sum::<f32>() / FRAME as f32).sqrt())
        .collect();
    if frames.len() < ENOUGH {
        return Level::default();
    }
    frames.sort_by(f32::total_cmp);
    Level { middle: frames[frames.len() / 2], loud: frames[frames.len() * 9 / 10] }
}

/// The samples out of a wav, wherever the writer put them.
fn data(wav: &[u8]) -> Option<&[u8]> {
    if wav.len() < 12 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12;
    while at + 8 <= wav.len() {
        let kind = &wav[at..at + 4];
        let long = u32::from_le_bytes([wav[at + 4], wav[at + 5], wav[at + 6], wav[at + 7]]) as usize;
        let from = at + 8;
        let to = from.saturating_add(long).min(wav.len());
        if kind == b"data" {
            return Some(&wav[from..to]);
        }
        at = from + long + (long & 1);
    }
    None
}

/// How short a recording is not worth measuring: a fifth of a second.
///
/// Two presses that close together are a thumb slipping, not a sentence, and a
/// handful of frames has no middle worth calling one.
const ENOUGH: usize = 10;

/// How far the loud of a recording has to stand above the quiet of it before
/// it is somebody talking.
///
/// This machine, measured: silence at every gain it has sits between 1.3 and
/// 1.9, and speech at 10.9. Two and a half is between them, nearer the room,
/// because of which mistake costs more.
pub const SPEAKS: f32 = 2.5;

/// The level at which a recording is talking however flat it is.
///
/// The measure above asks whether the loud parts stand above the quiet parts,
/// which is a question a recording of somebody talking without drawing breath
/// answers wrongly. So there is a second way to be speech: louder, all the way
/// through, than any room this machine has ever recorded. The loudest was its
/// own microphone at full boost hearing nothing, at thirteen per cent.
pub const LOUD: f32 = 0.20;

/// Whether there is anything in a recording worth asking about.
///
/// It is a shape rather than a level, because a level does not survive the gain
/// knob. Silence on this machine is 0.4 per cent of full scale at one boost
/// setting and 14 per cent at another -- so any line drawn across the level is
/// either above a real sentence at the quiet end or below an empty room at the
/// loud end. What does not move is that a room is the same all the way through
/// and a person is not.
pub fn anything_said(wav: &[u8]) -> bool {
    let heard = level(wav);
    (heard.loud > 0.0 && heard.loud >= heard.middle * SPEAKS) || heard.middle >= LOUD
}

/// What was heard, as a person would have typed it.
///
/// Whisper writes a line per stretch of speech and marks what is not speech at
/// all, so a room with nobody talking in it comes back as `[BLANK_AUDIO]`
/// rather than as nothing. Those marks are what somebody would have to delete
/// by hand, on a device with no keyboard, which is the thing this exists to
/// avoid.
///
/// And if what is left is short, the punctuation goes the same way and for the
/// same reason. See [`plainly`].
pub fn tidy(heard: &str) -> String {
    let words: Vec<&str> = heard
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_a_noise(line))
        .collect();
    plainly(&words.join(" ").split_whitespace().collect::<Vec<&str>>().join(" "))
}

/// Whether a line is whisper describing the room rather than quoting it.
fn is_a_noise(line: &str) -> bool {
    let bracketed = |open: char, close: char| line.starts_with(open) && line.ends_with(close);
    bracketed('[', ']') || bracketed('(', ')') || bracketed('*', '*')
}

/// How many words a thing can be and still be a name rather than a sentence.
///
/// Six, which is a guess at where one turns into the other and is meant to be.
/// The two mistakes are not the same size: a stop left on a search term is a
/// wrong search, and a stop taken off a message is a message somebody reads
/// anyway.
pub const SHORT: usize = 6;

/// What was said, with the marks off it if it was short enough to want none.
///
/// Whisper writes prose. Everything it hears is a sentence to it, so it ends
/// everything with a full stop and puts commas through the middle -- which is
/// right for a paragraph and wrong for nearly everything the paddle on the
/// back of this device is pressed for. What is dictated into it is a search
/// box, a filename, a name to look somebody up by. "Settings." looks for a
/// word this machine does not have, and the stop is then a backspace to be
/// found on a device whose whole problem is that it has no keyboard.
///
/// So the short things come back bare and the long ones are left alone.
/// Past [`SHORT`] words somebody is dictating a message rather than naming a
/// thing, and a message with its stops taken out is one long run of words that
/// they would have to punctuate by hand: the same chore, the other way round.
pub fn plainly(said: &str) -> String {
    if spoken(said) > SHORT {
        return said.to_string();
    }
    let letters: Vec<char> = said.chars().collect();
    let bare: String = letters
        .iter()
        .enumerate()
        .map(|(at, one)| match is_a_word(*one, &letters, at) {
            true => *one,
            false => ' ',
        })
        .collect();
    bare.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Whether a character is part of what was said rather than a mark upon it.
///
/// Letters and numbers, in any script, and the marks that stand on a letter
/// rather than beside it. And an apostrophe or a hyphen with a letter on either
/// side of it, because between two letters those are spelling and not
/// punctuation: "don't" and "well-known" are one word each, and a rule that
/// pulled the marks out of them would be a rule that misspells things rather
/// than one that stops writing full stops into a search box.
///
/// A mark is turned into a space rather than deleted, so that what was on both
/// sides of it stays two words. "one, two" is not "one,two" with the comma
/// gone.
fn is_a_word(one: char, said: &[char], at: usize) -> bool {
    if one.is_alphanumeric() || one.is_whitespace() || is_upon_a_letter(one) {
        return true;
    }
    let letter = |one: Option<&char>| one.is_some_and(|one| one.is_alphanumeric());
    let inside_a_word = at > 0 && letter(said.get(at - 1)) && letter(said.get(at + 1));
    matches!(one, '\'' | '\u{2019}' | '-') && (inside_a_word || starts_a_word(said, at))
}

/// Whether an apostrophe is the front of a word rather than a quote around one.
///
/// Dutch writes several of its commonest words that way -- `'s ochtends`, `'t
/// huis`, `'n beetje` -- and the rule above sees only that nothing letterish
/// stands to the left of the mark, which is also what it sees at the front of a
/// quotation. So it took the apostrophe off, and `'s ochtends` was typed as `s
/// ochtends`: the Thai fault again, in another language and a smaller way.
///
/// Told apart by how much follows. These are an apostrophe, one letter, and
/// then the end of the word; a quotation is an apostrophe and then a word. That
/// keeps `'s` and leaves the mark on `'hello'`, which is the way round that
/// matters, because one of the two is a word somebody said and the other is a
/// mark nobody dictated.
fn starts_a_word(said: &[char], at: usize) -> bool {
    let boundary = |one: Option<&char>| one.is_none_or(|one| !one.is_alphanumeric());
    boundary(said.get(at.wrapping_sub(1)).filter(|_| at > 0))
        && said.get(at + 1).is_some_and(|one| one.is_alphabetic())
        && boundary(said.get(at + 2))
}

/// Whether a character is a mark that stands on a letter rather than beside it.
///
/// Thai writes its vowels and its tones as marks upon the consonants, and to
/// anything asking whether a character is a letter they are not: they are
/// nonspacing marks, which is the same answer a comma gets. So the rule above,
/// left to itself, takes the vowels out of every Thai word it is given and
/// breaks what is left into pieces at each place one stood. "สวัสดีค่ะ" came
/// back as "สว สด ค ะ" -- four fragments of a word, none of them a word, typed
/// into whatever was waiting for it.
///
/// It is every short thing said in Thai, which is what this paddle is mostly
/// pressed for, failing in the way that is hardest to see: something arrives,
/// it is in the right script, and it is not what was said.
fn is_upon_a_letter(one: char) -> bool {
    ('\u{0e31}'..='\u{0e3a}').contains(&one) || ('\u{0e47}'..='\u{0e4e}').contains(&one)
}

/// How many words were said.
///
/// Gaps, mostly. Thai is the exception this machine hears: it puts no spaces
/// between its words at all -- they go between phrases instead -- so a
/// sentence of it is one word to anything counting gaps and would come back
/// stripped however long it ran. The characters of a script written without
/// spaces count for themselves instead, which is not one word each but is the
/// right order of magnitude, and what this number is asked is only which side
/// of a line it falls.
fn spoken(said: &str) -> usize {
    said.split_whitespace().map(|word| word.chars().filter(unspaced).count().max(1)).sum()
}

/// Whether a character belongs to a script that puts no spaces in.
fn unspaced(one: &char) -> bool {
    ('\u{0e00}'..='\u{0e7f}').contains(one)
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
        let (whisper, model) = (Path::new("/keep/whisper-cli"), Path::new("/keep/model.bin"));
        let argv = hearing(whisper, model, Path::new("/run/said.wav"), "auto");
        assert_eq!(argv.first().map(String::as_str), Some("/keep/whisper-cli"));
        assert!(argv.windows(2).any(|pair| pair == ["--model", "/keep/model.bin"]));
        assert!(argv.windows(2).any(|pair| pair == ["--file", "/run/said.wav"]));
        assert!(argv.iter().any(|word| word == "--no-timestamps"));
        assert!(argv.iter().any(|word| word == "--no-prints"));
    }

    /// The whole of what the setting does: whatever was chosen is what the
    /// hearing is told, and `auto` is a choice like the others rather than a
    /// separate way of asking.
    #[test]
    fn the_hearing_listens_for_the_language_that_was_chosen() {
        for language in &languages::EVERY {
            let argv = hearing(Path::new("w"), Path::new("m"), Path::new("s"), language.key);
            assert!(
                argv.windows(2).any(|pair| pair == ["--language", language.key]),
                "{} was not asked for",
                language.says
            );
        }
    }

    /// A double dash, because what was said is a sentence and a sentence can
    /// start with a word wtype would read as a flag.
    #[test]
    fn what_is_typed_is_handed_over_as_words_and_not_as_options() {
        assert_eq!(typing("-n is a flag"), ["wtype", "--", "-n is a flag"]);
    }

    #[test]
    fn what_was_heard_comes_back_as_one_line() {
        let said = "  Hello there, it has been a while.\n\n  How have you been keeping?  \n";
        assert_eq!(tidy(said), "Hello there, it has been a while. How have you been keeping?");
    }

    /// The paddle is pressed at a search box far more often than at a
    /// paragraph, and whisper ends everything it hears with a full stop.
    #[test]
    fn a_short_thing_comes_back_with_no_marks_on_it() {
        assert_eq!(tidy("Settings."), "Settings");
        assert_eq!(tidy("Blåhaj, please?"), "Blåhaj please");
        assert_eq!(tidy("\"Where is it?\""), "Where is it");
    }

    /// Long enough to be a sentence is long enough to want the stops kept: a
    /// message stripped of them is a message to be punctuated by hand.
    #[test]
    fn a_sentence_keeps_the_marks_it_came_with() {
        let said = "I'll be there at six, but I might be late, so don't wait for me.";
        assert_eq!(tidy(said), said);
    }

    /// A mark between two letters is spelling.
    #[test]
    fn the_marks_inside_words_are_part_of_the_words() {
        assert_eq!(tidy("don't -- well-known, isn't it?"), "don't well-known isn't it");
    }

    /// Thai writes no spaces between its words, so a sentence of it is one word
    /// to anything counting gaps and would never be long enough.
    #[test]
    fn a_language_with_no_spaces_is_counted_by_its_characters() {
        let said = "ฉันไปตลาดเมื่อเช้านี้ และซื้อผลไม้มาด้วย ทั้งหมดสดมาก";
        assert_eq!(tidy(said), said);
    }

    /// Thai hangs its vowels and tones on the consonants, and nothing asking
    /// whether a character is a letter says yes to those. Taking them out is
    /// not a full stop off a search term: it is the word misspelled, in
    /// pieces, and typed anyway.
    #[test]
    fn the_marks_a_thai_word_is_written_with_are_part_of_the_word() {
        assert_eq!(tidy("สวัสดีค่ะ"), "สวัสดีค่ะ");
        assert_eq!(tidy("ขอบคุณ"), "ขอบคุณ");
        assert_eq!(tidy("ไปไหน?"), "ไปไหน");
    }

    /// Dutch writes its plurals of vowel-final words with an apostrophe, and
    /// that apostrophe stands between two letters, which is the rule that
    /// keeps "don't" whole. It is the same rule and it is worth its own test,
    /// because "autos" and "fotos" are misspellings rather than plain words.
    #[test]
    fn a_dutch_plural_keeps_the_apostrophe_that_makes_it_one() {
        assert_eq!(tidy("Ik heb twee auto's."), "Ik heb twee auto's");
        assert_eq!(tidy("Waar zijn de foto's?"), "Waar zijn de foto's");
    }

    /// Dutch puts an apostrophe at the front of several of its commonest
    /// words, where the rule for "don't" cannot see it: nothing letterish
    /// stands to the left of the mark. So they were typed with the mark gone,
    /// which is the Thai fault again in a smaller way -- a real word, spelled
    /// wrong, typed anyway.
    #[test]
    fn a_dutch_word_that_begins_with_an_apostrophe_keeps_it() {
        assert_eq!(tidy("'s Ochtends drink ik koffie"), "'s Ochtends drink ik koffie");
        assert_eq!(tidy("'t Is koud vandaag"), "'t Is koud vandaag");
        assert_eq!(tidy("Geef me 'n moment."), "Geef me 'n moment");
    }

    /// And a mark that is a quotation rather than a word still goes. The two
    /// are told apart by what follows: one letter and the end of the word, or
    /// a whole word.
    #[test]
    fn a_quotation_is_not_a_word_that_begins_with_an_apostrophe() {
        assert_eq!(tidy("'hello' she said"), "hello she said");
    }

    /// Six is the line and the line is a guess, so both sides of it are
    /// written down: a thing that is asked for and a thing that is said.
    #[test]
    fn the_line_between_a_name_and_a_sentence_falls_where_it_says_it_does() {
        let asked = "Waar heb ik de sleutels gelaten?";
        assert_eq!(tidy(asked), "Waar heb ik de sleutels gelaten", "six words is a name");
        let said = "Waar heb ik de blauwe sleutels gelaten?";
        assert_eq!(tidy(said), said, "seven words is a sentence");
    }

    /// The marks Thai hangs on its letters, one of each kind that is not a
    /// letter to anything counting them: the silent-letter mark on the end of
    /// a word, and a tone mark under a vowel that is itself written as a mark.
    #[test]
    fn the_thai_marks_that_are_not_letters_survive_a_short_thing() {
        assert_eq!(tidy("จันทร์"), "จันทร์", "the silent-letter mark");
        assert_eq!(tidy("สัตว์"), "สัตว์", "the same, after a vowel mark");
        assert_eq!(tidy("น้ำ"), "น้ำ", "a tone mark and a sara am");
    }

    /// And a mark that really is punctuation still goes, in Thai as in
    /// anything else. The question is only which marks are which.
    #[test]
    fn a_thai_word_still_loses_the_punctuation_beside_it() {
        assert_eq!(tidy("น้ำ?"), "น้ำ");
    }

    /// Three words that differ only in tone, which is the clip the models are
    /// expected to disagree about. Here it is only the counting: eight
    /// characters across three words is past the line, so nothing is stripped.
    #[test]
    fn a_thai_line_is_counted_by_characters_across_the_gaps() {
        assert_eq!(tidy("หมา ม้า มา"), "หมา ม้า มา");
    }

    /// A room with nobody talking in it is nothing said, not a word to type.
    #[test]
    fn the_marks_for_what_is_not_speech_are_not_words() {
        assert_eq!(tidy("[BLANK_AUDIO]"), "");
        assert_eq!(tidy("(wind)\nHello\n*laughs*"), "Hello");
    }

    /// A recording of the shape `pw-record` is told to make, so the walking of
    /// the header is asked the same question the device asks it.
    fn a_wav(samples: &[i16]) -> Vec<u8> {
        let sound: Vec<u8> = samples.iter().flat_map(|one| one.to_le_bytes()).collect();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + sound.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // pcm
        wav.extend_from_slice(&1u16.to_le_bytes()); // one channel
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&32000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(sound.len() as u32).to_le_bytes());
        wav.extend_from_slice(&sound);
        wav
    }

    /// A room: the same level all the way through, wherever the gain is set.
    fn a_room(level: i16) -> Vec<i16> {
        (0..16000).map(|at| if at % 2 == 0 { level } else { -level }).collect()
    }

    /// Somebody talking: quiet, then not, then quiet again.
    fn a_sentence(room: i16, voice: i16) -> Vec<i16> {
        let mut said = a_room(room);
        for (at, one) in said[4000..9000].iter_mut().enumerate() {
            *one = if at % 2 == 0 { voice } else { -voice };
        }
        said
    }

    #[test]
    fn the_quiet_and_the_loud_of_a_recording_are_measured_in_frames() {
        let heard = level(&a_wav(&a_room(3277)));
        assert!((heard.middle - 0.1).abs() < 0.001, "{heard:?}");
        assert!((heard.loud - 0.1).abs() < 0.001, "{heard:?}");
    }

    /// A wav is chunks, and a writer may put its own before the samples.
    #[test]
    fn the_samples_are_found_past_whatever_else_the_writer_wrote() {
        let mut wav = a_wav(&a_sentence(300, 9000));
        let extra = b"LIST\x04\x00\x00\x00abcd";
        wav.splice(12..12, extra.iter().copied());
        assert!(anything_said(&wav));
    }

    /// Nothing that is not a recording is a sentence.
    #[test]
    fn what_is_not_a_recording_is_not_a_sentence() {
        assert_eq!(level(b""), Level::default());
        assert_eq!(level(b"this is not a wav at all"), Level::default());
        assert!(!anything_said(b""));
    }

    /// The point of the guard: whisper answers an empty room with "Thank you."
    #[test]
    fn a_room_with_nobody_in_it_is_not_asked_about() {
        assert!(!anything_said(&a_wav(&a_room(0))));
        assert!(!anything_said(&a_wav(&a_room(300))));
        assert!(!anything_said(&a_wav(&a_sentence(300, 400))));
    }

    /// The measured thing: the same room at four gains, and the same sentence.
    /// A line drawn across the level would have to fall between them all.
    #[test]
    fn the_gain_knob_moves_the_room_and_not_the_answer() {
        for level in [118, 455, 1541, 4260] {
            // Capped, because a room already at a tenth of full scale leaves
            // nowhere for a voice eight times louder to go.
            let voice = (i32::from(level) * 8).min(30_000) as i16;
            assert!(!anything_said(&a_wav(&a_room(level))), "{level} is a room");
            assert!(anything_said(&a_wav(&a_sentence(level, voice))), "{level} is spoken");
        }
    }

    /// Somebody talking without drawing breath is flat, and still speech.
    #[test]
    fn talking_all_the_way_through_is_talking() {
        assert!(anything_said(&a_wav(&a_room(9000))));
    }

    /// Two presses a moment apart are a thumb slipping.
    #[test]
    fn a_recording_too_short_to_have_a_middle_is_nothing_said() {
        assert!(!anything_said(&a_wav(&a_room(9000)[..1000])));
    }

    /// Pointed at the graphics card, and built as one file.
    #[test]
    fn the_hearing_is_built_for_the_card_this_machine_has() {
        let argv = configuring(Path::new("/run/whisper.cpp"));
        assert!(argv.iter().any(|word| word == "-DGGML_VULKAN=ON"));
        assert!(argv.iter().any(|word| word == "-DBUILD_SHARED_LIBS=OFF"));
    }

    /// A compiler pointed at somebody else's repository is pointed at a tag.
    #[test]
    fn the_source_of_the_hearing_is_pinned() {
        let argv = cloning(Path::new("/run/whisper.cpp"));
        assert!(argv.windows(2).any(|pair| pair == ["--branch", WHISPER_AT]));
        assert!(argv.iter().any(|word| word == WHISPER_FROM));
        assert!(WHISPER_AT.starts_with('v'), "a tag, not a branch");
    }

    /// It is made where a session ending clears it, and kept where one does not.
    #[test]
    fn the_build_tree_does_not_outlive_the_build() {
        // SAFETY: one thread, and both variables are put back before it ends.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        unsafe { std::env::set_var("XDG_DATA_HOME", "/home/somebody/.local/share") };
        assert!(making().starts_with("/run/user/1000"));
        assert!(whisper().starts_with("/home/somebody/.local/share"));
        assert_eq!(whisper().parent(), model().parent(), "beside the model");
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }

    #[test]
    fn the_recording_waits_where_a_session_ending_clears_it() {
        // SAFETY: one thread, and both variables are put back before it ends.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        unsafe { std::env::set_var("XDG_DATA_HOME", "/home/somebody/.local/share") };
        assert_eq!(said(41), Path::new("/run/user/1000/console/voice/said-41.wav"));
        assert_eq!(taking(), Path::new("/run/user/1000/console/voice/taking.pid"));
        assert_eq!(model(), Path::new("/home/somebody/.local/share/console/voice").join(MODEL));
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }

    /// The whole of the fix: no two presses write to the same file, so the one
    /// tidying up after a recording can never delete the one being made.
    #[test]
    fn two_presses_do_not_share_a_recording() {
        assert_ne!(said(41), said(42));
    }

    #[test]
    fn the_note_says_who_to_stop_and_what_to_read() {
        assert_eq!(taken(1234, 99), "1234 99");
        assert_eq!(told_by("1234 99"), Some((1234, 99)));
        assert_eq!(told_by(" 1234  99 \n"), Some((1234, 99)));
    }

    /// A note left by the build before this one names no recording, and the
    /// one it meant is not written any more.
    #[test]
    fn a_note_that_names_no_recording_is_refused() {
        assert_eq!(told_by("1234"), None);
        assert_eq!(told_by(""), None);
        assert_eq!(told_by("not a number at all"), None);
    }
}

